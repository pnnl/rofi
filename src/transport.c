/*
 * Copyright (c) 2013-2018 Intel Corporation.  All rights reserved.
 * Copyright (c) 2016 Cray Inc.  All rights reserved.
 * Copyright (c) 2014-2017, Cisco Systems, Inc. All rights reserved.
 *
 * This software is available to you under the BSD license below:
 *
 *     Redistribution and use in source and binary forms, with or
 *     without modification, are permitted provided that the following
 *     conditions are met:
 *
 *      - Redistributions of source code must retain the above
 *        copyright notice, this list of conditions and the following
 *        disclaimer.
 *
 *      - Redistributions in binary form must reproduce the above
 *        copyright notice, this list of conditions and the following
 *        disclaimer in the documentation and/or other materials
 *        provided with the distribution.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
 * EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
 * MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
 * NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS
 * BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN
 * ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN
 * CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 * SOFTWARE.
 */

#include <assert.h>
#include <netdb.h>
#include <poll.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>
#include <sys/wait.h>
#include <sys/types.h>
#include <sys/socket.h>

#include <rdma/fi_cm.h>
#include <rdma/fi_domain.h>
#include <rdma/fi_errno.h>
#include <rdma/fi_endpoint.h>
#include <rdma/fi_rma.h>
#include <rdma/fi_tagged.h>
#include <rdma/fi_atomic.h>
#include <rdma/fi_collective.h>

#include <rofi_debug.h>
#include <rofi_internal.h>
#include <transport.h>


struct fi_info *fi_pep, *fi, *hints;
struct fid_fabric *fabric;
struct fid_wait *waitset;
struct fid_domain *domain;
struct fid_poll *pollset;
struct fid_pep *pep;
struct fid_ep *ep, *alias_ep;
struct fid_cq *txcq, *rxcq;
struct fid_cntr *txcntr, *rxcntr;
struct fid_mr *mr;
void *mr_desc = NULL;
struct fid_av *av;
struct fid_eq *eq;
struct fid_mc *mc;


struct fid_mr no_mr;
struct fi_context tx_ctx, rx_ctx;
struct ft_context *tx_ctx_arr = NULL, *rx_ctx_arr = NULL;
uint64_t remote_cq_data = 0;

uint64_t tx_seq, rx_seq, tx_cq_cntr, rx_cq_cntr;
int (*ft_mr_alloc_func)(void);
uint64_t ft_tag = 0;
int ft_parent_proc = 0;
pid_t ft_child_pid = 0;
int ft_socket_pair[2];

//fi_addr_t remote_fi_addr = {FI_ADDR_UNSPEC};

extern fi_addr_t* remote_fi_addrs;

char *buf, *tx_buf, *rx_buf;
char **tx_mr_bufs = NULL, **rx_mr_bufs = NULL;
size_t buf_size, tx_size, rx_size, tx_mr_size, rx_mr_size;
int rx_fd = -1, tx_fd = -1;
char default_port[8] = "9228";
static char default_oob_port[8] = "3000";
const char *greeting = "Hello from Client!";


char test_name[50] = "custom";
int timeout = -1;
struct timespec start, end;

int listen_sock = -1;
int sock = -1;
int oob_sock = -1;

struct fi_av_attr av_attr = {
	.type = FI_AV_MAP,
	.count = 1
};
struct fi_eq_attr eq_attr = {
	.wait_obj = FI_WAIT_UNSPEC
};
struct fi_cq_attr cq_attr = {
	.wait_obj = FI_WAIT_NONE
};
struct fi_cntr_attr cntr_attr = {
	.events = FI_CNTR_EVENTS_COMP,
	.wait_obj = FI_WAIT_NONE
};

struct ft_opts opts;

struct test_size_param test_size[] = {
	{ 1 <<  0, 0 },
	{ 1 <<  1, 0 }, { (1 <<  1) + (1 <<  0), 0 },
	{ 1 <<  2, 0 }, { (1 <<  2) + (1 <<  1), 0 },
	{ 1 <<  3, 0 }, { (1 <<  3) + (1 <<  2), 0 },
	{ 1 <<  4, 0 }, { (1 <<  4) + (1 <<  3), 0 },
	{ 1 <<  5, 0 }, { (1 <<  5) + (1 <<  4), 0 },
	{ 1 <<  6, FT_DEFAULT_SIZE }, { (1 <<  6) + (1 <<  5), 0 },
	{ 1 <<  7, 0 }, { (1 <<  7) + (1 <<  6), 0 },
	{ 1 <<  8, FT_DEFAULT_SIZE }, { (1 <<  8) + (1 <<  7), 0 },
	{ 1 <<  9, 0 }, { (1 <<  9) + (1 <<  8), 0 },
	{ 1 << 10, FT_DEFAULT_SIZE }, { (1 << 10) + (1 <<  9), 0 },
	{ 1 << 11, 0 }, { (1 << 11) + (1 << 10), 0 },
	{ 1 << 12, FT_DEFAULT_SIZE }, { (1 << 12) + (1 << 11), 0 },
	{ 1 << 13, 0 }, { (1 << 13) + (1 << 12), 0 },
	{ 1 << 14, 0 }, { (1 << 14) + (1 << 13), 0 },
	{ 1 << 15, 0 }, { (1 << 15) + (1 << 14), 0 },
	{ 1 << 16, FT_DEFAULT_SIZE }, { (1 << 16) + (1 << 15), 0 },
	{ 1 << 17, 0 }, { (1 << 17) + (1 << 16), 0 },
	{ 1 << 18, 0 }, { (1 << 18) + (1 << 17), 0 },
	{ 1 << 19, 0 }, { (1 << 19) + (1 << 18), 0 },
	{ 1 << 20, FT_DEFAULT_SIZE }, { (1 << 20) + (1 << 19), 0 },
	{ 1 << 21, 0 }, { (1 << 21) + (1 << 20), 0 },
	{ 1 << 22, 0 }, { (1 << 22) + (1 << 21), 0 },
	{ 1 << 23, 0 },
};

const unsigned int test_cnt = (sizeof test_size / sizeof test_size[0]);

#define INTEG_SEED 7
static const char integ_alphabet[] = "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
static const int integ_alphabet_length = (sizeof(integ_alphabet)/sizeof(*integ_alphabet)) - 1;

struct fid_av_set *av_set;
fi_addr_t coll_addr;
struct fid_mc *coll_mc;
fi_addr_t world_addr;

static int wait_for_event(uint32_t event)
{
	uint32_t ev;
	int err;
	struct fi_cq_err_entry comp = { 0 };

	do {
		err = fi_eq_read(eq, &ev, NULL, 0, 0);
		if (err >= 0) {
			FT_DEBUG("found eq entry %d\n", event);
			if (ev == event) {
				return FI_SUCCESS;
			}
		} else if (err != -EAGAIN) {
			return err;
		}

		err = fi_cq_read(rxcq, &comp, 1);
		if (err < 0 && err != -EAGAIN) {
			return err;
		}

		err = fi_cq_read(txcq, &comp, 1);
		if (err < 0 && err != -EAGAIN) {
			return err;
		}
	} while (err == -FI_EAGAIN);

	return err;
}

static int coll_setup()
{
	int err;
	struct fi_av_set_attr av_set_attr;
	struct fi_collective_attr attr;

	attr.op = FI_NOOP;
	attr.datatype = FI_UINT64;
	attr.mode = 0;
	err = fi_query_collective(domain, FI_ALLGATHER, &attr, 0);
	if (err) {
		FT_DEBUG("SUM AllReduce collective not supported: %d (%s)\n", err,
			 fi_strerror(err));
		return err;
	}



	av_set_attr.count = rt_get_size();
	av_set_attr.start_addr = 0;
	av_set_attr.end_addr = rt_get_size() - 1;
	av_set_attr.stride = 1;

	err = fi_av_set(av, &av_set_attr, &av_set, NULL);
	if (err) {
		FT_DEBUG("av_set creation failed ret = %d\n", err);
	}

	err = fi_av_set_addr(av_set, &world_addr);
	if (err) {
		FT_DEBUG("failed to get collective addr = %d (%s)\n", err,
			 fi_strerror(err));
		return err;
	}

	err = fi_join_collective(ep, world_addr, av_set, 0, &coll_mc, NULL);
	if (err) {
		FT_DEBUG("collective join failed ret = %d (%s)\n", err, fi_strerror(err));
		return err;
	}

	return wait_for_event(FI_JOIN_COMPLETE);
}

static int ft_poll_fd(int fd, int timeout)
{
	struct pollfd fds;
	int ret;

	fds.fd = fd;
	fds.events = POLLIN;
	ret = poll(&fds, 1, timeout);
	if (ret == -1) {
		FT_PRINTERR("poll", -errno);
		ret = -errno;
	} else if (!ret) {
		ret = -FI_EAGAIN;
	} else {
		ret = 0;
	}
	return ret;
}

size_t ft_tx_prefix_size(void)
{
	return (fi->tx_attr->mode & FI_MSG_PREFIX) ?
		fi->ep_attr->msg_prefix_size : 0;
}

size_t ft_rx_prefix_size(void)
{
	return (fi->rx_attr->mode & FI_MSG_PREFIX) ?
		fi->ep_attr->msg_prefix_size : 0;
}

int ft_check_opts(uint64_t flags)
{
	return (opts.options & flags) == flags;
}

static void ft_cq_set_wait_attr(void)
{
	switch (opts.comp_method) {
	case FT_COMP_SREAD:
		cq_attr.wait_obj = FI_WAIT_UNSPEC;
		cq_attr.wait_cond = FI_CQ_COND_NONE;
		break;
	case FT_COMP_WAITSET:
		assert(waitset);
		cq_attr.wait_obj = FI_WAIT_SET;
		cq_attr.wait_cond = FI_CQ_COND_NONE;
		cq_attr.wait_set = waitset;
		break;
	case FT_COMP_WAIT_FD:
		cq_attr.wait_obj = FI_WAIT_FD;
		cq_attr.wait_cond = FI_CQ_COND_NONE;
		break;
	case FT_COMP_YIELD:
		cq_attr.wait_obj = FI_WAIT_YIELD;
		cq_attr.wait_cond = FI_CQ_COND_NONE;
		break;
	default:
		cq_attr.wait_obj = FI_WAIT_NONE;
		break;
	}
}

static void ft_cntr_set_wait_attr(void)
{
	switch (opts.comp_method) {
	case FT_COMP_SREAD:
		cntr_attr.wait_obj = FI_WAIT_UNSPEC;
		break;
	case FT_COMP_WAITSET:
		assert(waitset);
		cntr_attr.wait_obj = FI_WAIT_SET;
		break;
	case FT_COMP_WAIT_FD:
		cntr_attr.wait_obj = FI_WAIT_FD;
		break;
	case FT_COMP_YIELD:
		cntr_attr.wait_obj = FI_WAIT_YIELD;
		break;
	default:
		cntr_attr.wait_obj = FI_WAIT_NONE;
		break;
	}
}

int ft_cntr_open(struct fid_cntr **cntr)
{
	ft_cntr_set_wait_attr();
	return fi_cntr_open(domain, &cntr_attr, cntr, cntr);
}

static inline int ft_rma_read_target_allowed(uint64_t caps)
{
	if (caps & (FI_RMA | FI_ATOMIC)) {
		if (caps & FI_REMOTE_READ)
			return 1;
		return !(caps & (FI_READ | FI_WRITE | FI_REMOTE_WRITE));
	}
	return 0;
}

static inline int ft_rma_write_target_allowed(uint64_t caps)
{
	if (caps & (FI_RMA | FI_ATOMIC)) {
		if (caps & FI_REMOTE_WRITE)
			return 1;
		return !(caps & (FI_READ | FI_WRITE | FI_REMOTE_WRITE));
	}
	return 0;
}

static inline int ft_check_mr_local_flag(struct fi_info *info)
{
	return ((info->mode & FI_LOCAL_MR) ||
		(info->domain_attr->mr_mode & FI_MR_LOCAL));
}

uint64_t ft_info_to_mr_access(struct fi_info *info)
{
	uint64_t mr_access = 0;
	if (ft_check_mr_local_flag(info)) {
		if (info->caps & (FI_MSG | FI_TAGGED)) {
			if (info->caps & FT_MSG_MR_ACCESS) {
				mr_access |= info->caps & FT_MSG_MR_ACCESS;
			} else {
				mr_access |= FT_MSG_MR_ACCESS;
			}
		}

		if (info->caps & (FI_RMA | FI_ATOMIC)) {
			if (info->caps & FT_RMA_MR_ACCESS) {
				mr_access |= info->caps & FT_RMA_MR_ACCESS;
			} else	{
				mr_access |= FT_RMA_MR_ACCESS;
			}
		}
	} else {
		if (info->caps & (FI_RMA | FI_ATOMIC)) {
			if (ft_rma_read_target_allowed(info->caps)) {
				mr_access |= FI_REMOTE_READ;
			}
			if (ft_rma_write_target_allowed(info->caps)) {
				mr_access |= FI_REMOTE_WRITE;
			}
		}
	}
	return mr_access;
}

#define bit_isset(x, i) (x >> i & 1)
#define for_each_bit(x, i) for (i = 0; i < (8 * sizeof(x)); i++)

static inline int bit_set_count(uint64_t val)
{
	int cnt = 0;
	while (val) {
		cnt++;
		val &= val - 1;
	}
	return cnt;
}

int ft_alloc_bit_combo(uint64_t fixed, uint64_t opt,
		       uint64_t **combos, int *len)
{
	uint64_t *flags;
	int i, num_flags;
	uint64_t index;
	int ret;

	num_flags = bit_set_count(opt) + 1;
	flags = calloc(num_flags, sizeof(fixed));
	if (!flags) {
		perror("calloc");
		return -FI_ENOMEM;
	}

	*len = 1 << (num_flags - 1);
	*combos = calloc(*len, sizeof(fixed));
	if (!(*combos)) {
		perror("calloc");
		ret = -FI_ENOMEM;
		goto clean;
	}

	num_flags = 0;
	for_each_bit(opt, i) {
		if (bit_isset(opt, i))
			flags[num_flags++] = 1ULL << i;
	}

	for (index = 0; index < (*len); index++) {
		(*combos)[index] = fixed;
		for_each_bit(index, i) {
			if (bit_isset(index, i))
				(*combos)[index] |= flags[i];
		}
	}
	ret = FI_SUCCESS;

clean:
	free(flags);
	return ret;
}

void ft_free_bit_combo(uint64_t *combo)
{
	free(combo);
}

static int ft_alloc_ctx_array(struct ft_context **mr_array, char ***mr_bufs,
			      char *default_buf, size_t mr_size,
			      uint64_t start_key)
{
	int i, ret;
	uint64_t access = ft_info_to_mr_access(fi);
	struct ft_context *context;

	*mr_array = calloc(opts.window_size, sizeof(**mr_array));
	if (!*mr_array)
		return -FI_ENOMEM;

	if (opts.options & FT_OPT_ALLOC_MULT_MR) {
		*mr_bufs = calloc(opts.window_size, sizeof(**mr_bufs));
		if (!mr_bufs)
			return -FI_ENOMEM;
	}

	for (i = 0; i < opts.window_size; i++) {
		context = &(*mr_array)[i];
		if (!(opts.options & FT_OPT_ALLOC_MULT_MR)) {
			context->buf = default_buf + mr_size * i;
			continue;
		}
		(*mr_bufs)[i] = calloc(1, mr_size);
		context->buf = (*mr_bufs)[i];
    		if (((fi->domain_attr->mr_mode & FI_MR_LOCAL) ||
		     (fi->caps & (FI_RMA | FI_ATOMIC)))) {
			ret = fi_mr_reg(domain, context->buf,
					mr_size, access, 0,
					start_key + i, 0,
					&context->mr, NULL);
			if (ret)
				return ret;

			context->desc = fi_mr_desc(context->mr);
		} else {
			context->mr =  NULL;
			context->desc = NULL;
		}
	}

	return 0;
}

static void ft_set_tx_rx_sizes(size_t *set_tx, size_t *set_rx)
{
	*set_tx = opts.options & FT_OPT_SIZE ?
		  opts.transfer_size : test_size[TEST_CNT - 1].size;
	if (*set_tx > fi->ep_attr->max_msg_size)
		*set_tx = fi->ep_attr->max_msg_size;
	*set_rx = *set_tx + ft_rx_prefix_size();
	*set_tx += ft_tx_prefix_size();
}

/*
 * Include FI_MSG_PREFIX space in the allocated buffer, and ensure that the
 * buffer is large enough for a control message used to exchange addressing
 * data.
 */
static int ft_alloc_msgs(void)
{
	int ret;
	long alignment = 1;

	if (ft_check_opts(FT_OPT_SKIP_MSG_ALLOC))
		return 0;

	if (opts.options & FT_OPT_ALLOC_MULT_MR) {
		ft_set_tx_rx_sizes(&tx_mr_size, &rx_mr_size);
		rx_size = FT_MAX_CTRL_MSG + ft_rx_prefix_size();
		tx_size = FT_MAX_CTRL_MSG + ft_tx_prefix_size();
		buf_size = rx_size + tx_size;
	} else {
		ft_set_tx_rx_sizes(&tx_size, &rx_size);
		tx_mr_size = 0;
		rx_mr_size = 0;
		buf_size = MAX(tx_size, FT_MAX_CTRL_MSG) * opts.window_size + 
			   MAX(rx_size, FT_MAX_CTRL_MSG) * opts.window_size;
	}

	if (opts.options & FT_OPT_ALIGN) {
		alignment = sysconf(_SC_PAGESIZE);
		if (alignment < 0)
			return -errno;
		buf_size += alignment;

		ret = posix_memalign((void **) &buf, (size_t) alignment,
				buf_size);
		if (ret) {
			FT_PRINTERR("posix_memalign", ret);
			return ret;
		}
	} else {
		buf = malloc(buf_size);
		if (!buf) {
			perror("malloc");
			return -FI_ENOMEM;
		}
	}
	memset(buf, 0, buf_size);
	rx_buf = buf;

	if (opts.options & FT_OPT_ALLOC_MULT_MR)
		tx_buf = (char *) buf + MAX(rx_size, FT_MAX_CTRL_MSG);
	else
		tx_buf = (char *) buf + MAX(rx_size, FT_MAX_CTRL_MSG) * opts.window_size;

	remote_cq_data = ft_init_cq_data(fi);

	if (!ft_mr_alloc_func && !ft_check_opts(FT_OPT_SKIP_REG_MR) &&
	    ((fi->domain_attr->mr_mode & FI_MR_LOCAL) ||
	     (fi->caps & (FI_RMA | FI_ATOMIC)))) {
		ret = fi_mr_reg(domain, buf, buf_size, ft_info_to_mr_access(fi),
				0, FT_MR_KEY, 0, &mr, NULL);
		if (ret) {
			FT_PRINTERR("fi_mr_reg", ret);
			return ret;
		}
		mr_desc = ft_check_mr_local_flag(fi) ? fi_mr_desc(mr) : NULL;
	} else {
		if (ft_mr_alloc_func) {
			assert(!ft_check_opts(FT_OPT_SKIP_REG_MR));
			ret = ft_mr_alloc_func();
			if (ret)
				return ret;
		}
		mr = &no_mr;
	}

	ret = ft_alloc_ctx_array(&tx_ctx_arr, &tx_mr_bufs, tx_buf,
				 tx_mr_size, FT_TX_MR_KEY);
	if (ret)
		return -FI_ENOMEM;

	ret = ft_alloc_ctx_array(&rx_ctx_arr, &rx_mr_bufs, rx_buf,
				 rx_mr_size, FT_RX_MR_KEY);
	if (ret)
		return -FI_ENOMEM;

	return 0;
}

int ft_open_fabric_res(void)
{
	int ret;
	size_t eplen;

	ret = fi_fabric(fi->fabric_attr, &fabric, NULL);
	if (ret) {
		FT_PRINTERR("fi_fabric", ret);
		return ret;
	}

	ret = fi_eq_open(fabric, &eq_attr, &eq, NULL);
	if (ret) {
		FT_PRINTERR("fi_eq_open", ret);
		return ret;
	}

	ret = fi_domain(fabric, fi, &domain, NULL);
	if (ret) {
		FT_PRINTERR("fi_domain", ret);
		return ret;
	}

	return 0;
}

int ft_alloc_ep_res(struct fi_info *fi)
{
	int ret;

	ret = ft_alloc_msgs();
	if (ret)
		return ret;

	if (cq_attr.format == FI_CQ_FORMAT_UNSPEC) {
		if (fi->caps & FI_TAGGED)
			cq_attr.format = FI_CQ_FORMAT_TAGGED;
		else
			cq_attr.format = FI_CQ_FORMAT_CONTEXT;
	}

	if (opts.options & FT_OPT_CQ_SHARED) {
		ft_cq_set_wait_attr();
		cq_attr.size = 0;

		if (opts.tx_cq_size)
			cq_attr.size += opts.tx_cq_size;
		else
			cq_attr.size += fi->tx_attr->size;

		if (opts.rx_cq_size)
			cq_attr.size += opts.rx_cq_size;
		else
			cq_attr.size += fi->rx_attr->size;

		ret = fi_cq_open(domain, &cq_attr, &txcq, &txcq);
		if (ret) {
			FT_PRINTERR("fi_cq_open", ret);
			return ret;
		}
		rxcq = txcq;
	}

	if (!(opts.options & FT_OPT_CQ_SHARED)) {
		ft_cq_set_wait_attr();
		if (opts.tx_cq_size)
			cq_attr.size = opts.tx_cq_size;
		else
			cq_attr.size = fi->tx_attr->size;

		ret = fi_cq_open(domain, &cq_attr, &txcq, &txcq);
		if (ret) {
			FT_PRINTERR("fi_cq_open", ret);
			return ret;
		}
	}

	if (opts.options & FT_OPT_TX_CNTR) {
		ret = ft_cntr_open(&txcntr);
		if (ret) {
			FT_PRINTERR("fi_cntr_open", ret);
			return ret;
		}
	}

	if (!(opts.options & FT_OPT_CQ_SHARED)) {
		ft_cq_set_wait_attr();
		if (opts.rx_cq_size)
			cq_attr.size = opts.rx_cq_size;
		else
			cq_attr.size = fi->rx_attr->size;

		ret = fi_cq_open(domain, &cq_attr, &rxcq, &rxcq);
		if (ret) {
			FT_PRINTERR("fi_cq_open", ret);
			return ret;
		}
	}

	if (opts.options & FT_OPT_RX_CNTR) {
		ret = ft_cntr_open(&rxcntr);
		if (ret) {
			FT_PRINTERR("fi_cntr_open", ret);
			return ret;
		}
	}

	if (fi->ep_attr->type == FI_EP_RDM || fi->ep_attr->type == FI_EP_DGRAM) {
		if (fi->domain_attr->av_type != FI_AV_UNSPEC)
			av_attr.type = fi->domain_attr->av_type;

		if (opts.av_name) {
			av_attr.name = opts.av_name;
		}
		av_attr.count = opts.av_size;
		ret = fi_av_open(domain, &av_attr, &av, NULL);
		if (ret) {
			FT_PRINTERR("fi_av_open", ret);
			return ret;
		}
	}
	return 0;
}

int ft_alloc_active_res(struct fi_info *fi)
{
	int ret;

	ret = ft_alloc_ep_res(fi);
	if (ret)
		return ret;

	ret = fi_endpoint(domain, fi, &ep, NULL);
	if (ret) {
		FT_PRINTERR("fi_endpoint", ret);
		return ret;
	}
	
	return 0;
}

static void ft_init(void)
{
	tx_seq = 0;
	rx_seq = 0;
	tx_cq_cntr = 0;
	rx_cq_cntr = 0;
}

int ft_getinfo(struct fi_info *hints, struct fi_info **info)
{
	char *node, *service;
	uint64_t flags = 0;
	int ret;

	ret = fi_getinfo(FT_FIVERSION, NULL, NULL, flags, hints, info);
	if (ret) {
		FT_PRINTERR("fi_getinfo", ret);
		return ret;
	}

	return 0;
}

int ft_init_fabric(void)
{
	int ret, i;
	char name[512];
	char epname[512];
	size_t len = 64;

	ft_init();

	ret = ft_getinfo(hints, &fi);
	if (ret)
		return ret;

	ret = ft_open_fabric_res();
	if (ret)
		return ret;

	if(!strncmp(fi->fabric_attr->prov_name, "verbs", 5))
		rdesc.prov = verbs;
	else if(!strcmp(fi->fabric_attr->prov_name, "shm"))
		rdesc.prov = shm;
	else{
		ERR_MSG("Provider %s not supported. Aborting.", fi->fabric_attr->prov_name);
		return -1;
	}  
		

	DEBUG_MSG("\tSelected Provider: %s (%d) Version: (%u.%u) Fabric: %s Domain: %s max_inject: %zu, max_msg: %zu stx: %s MR_RMA_EVENT: %s src_addr %s src_addrlen %lu dest_addr %s dest_addrlen %lu",
		  fi->fabric_attr->prov_name, rdesc.prov,
		  FI_MAJOR(fi->fabric_attr->prov_version),
		  FI_MINOR(fi->fabric_attr->prov_version),
		  fi->fabric_attr->name,
		  fi->domain_attr->name,
		  fi->tx_attr->inject_size,
		  fi->ep_attr->max_msg_size,
		  fi->domain_attr->max_ep_stx_ctx == 0 ? "no" : "yes",
		  fi->domain_attr->mr_mode & FI_MR_RMA_EVENT ? "yes" : " no",
		  fi->src_addr, fi->src_addrlen,
		  fi->dest_addr, fi->dest_addrlen);

	ret = ft_alloc_active_res(fi);
	if (ret)
		return ret;

	ret = ft_enable_ep_recv();
	if (ret)
		return ret;

	ret = fi_getname(&ep->fid, epname, &len);
	if (ret) {
		FT_PRINTERR("fi_getname", ret);
		return ret;
	}

	rt_put("epname_len", &len, sizeof(size_t));
	rt_put("epname", epname, len);
	rdesc.addrlen = len;

	rt_exchange();

	ret = ft_init_av();
	if (ret)
		return ret;

	return 0;
}

int ft_get_cq_fd(struct fid_cq *cq, int *fd)
{
	int ret = FI_SUCCESS;

	if (cq && opts.comp_method == FT_COMP_WAIT_FD) {
		ret = fi_control(&cq->fid, FI_GETWAIT, fd);
		if (ret)
			FT_PRINTERR("fi_control(FI_GETWAIT)", ret);
	}

	return ret;
}

int ft_init_alias_ep(uint64_t flags)
{
	int ret;
	ret = fi_ep_alias(ep, &alias_ep, flags);
	if (ret) {
		FT_PRINTERR("fi_ep_alias", ret);
		return ret;
	}
	return 0;
}

int ft_enable_ep(struct fid_ep *ep, struct fid_eq *eq, struct fid_av *av,
		 struct fid_cq *txcq, struct fid_cq *rxcq,
		 struct fid_cntr *txcntr, struct fid_cntr *rxcntr)
{
	uint64_t flags;
	int ret;
	size_t len = 64;
	char epname[512];

	if (fi->ep_attr->type == FI_EP_MSG || fi->caps & FI_MULTICAST ||
	    fi->caps & FI_COLLECTIVE)
		FT_EP_BIND(ep, eq, 0);

	FT_EP_BIND(ep, av, 0);

	flags = FI_TRANSMIT;
	if (!(opts.options & FT_OPT_TX_CQ))
		flags |= FI_SELECTIVE_COMPLETION;
	FT_EP_BIND(ep, txcq, flags);

	flags = FI_RECV;
	if (!(opts.options & FT_OPT_RX_CQ))
		flags |= FI_SELECTIVE_COMPLETION;
	FT_EP_BIND(ep, rxcq, flags);

	ret = ft_get_cq_fd(txcq, &tx_fd);
	if (ret)
		return ret;

	ret = ft_get_cq_fd(rxcq, &rx_fd);
	if (ret)
		return ret;

	/* TODO: use control structure to select counter bindings explicitly */
	if (opts.options & FT_OPT_TX_CQ)
		flags = 0;
	else
		flags = FI_SEND;
	if (hints->caps & (FI_WRITE | FI_READ))
		flags |= hints->caps & (FI_WRITE | FI_READ);
	else if (hints->caps & FI_RMA)
		flags |= FI_WRITE | FI_READ;
	FT_EP_BIND(ep, txcntr, flags);

	if (opts.options & FT_OPT_RX_CQ)
		flags = 0;
	else
		flags = FI_RECV;
	if (hints->caps & (FI_REMOTE_WRITE | FI_REMOTE_READ))
		flags |= hints->caps & (FI_REMOTE_WRITE | FI_REMOTE_READ);
	else if (hints->caps & FI_RMA)
		flags |= FI_REMOTE_WRITE | FI_REMOTE_READ;
	FT_EP_BIND(ep, rxcntr, flags);

	ret = fi_enable(ep);
	if (ret) {
		FT_PRINTERR("fi_enable", ret);
		return ret;
	}

	return 0;
}

int ft_enable_ep_recv(void)
{
	int ret;

	ret = ft_enable_ep(ep, eq, av, txcq, rxcq, txcntr, rxcntr);
	if (ret)
		return ret;

	if (!ft_check_opts(FT_OPT_SKIP_MSG_ALLOC) &&
	    (fi->caps & (FI_MSG | FI_TAGGED))) {
		/* Initial receive will get remote address for unconnected EPs */
		ret = ft_post_rx(ep, MAX(rx_size, FT_MAX_CTRL_MSG), &rx_ctx);
		if (ret)
			return ret;
	}

	return 0;
}

int ft_av_insert(struct fid_av *av, void *addr, size_t count, fi_addr_t *fi_addr,
		uint64_t flags, void *context)
{
	int ret;

	ret = fi_av_insert(av, addr, count, fi_addr, flags, context);
	if (ret < 0) {
		FT_PRINTERR("fi_av_insert", ret);
		return ret;
	} else if (ret != count) {
		FT_ERR("fi_av_insert: number of addresses inserted = %d;"
			       " number of addresses given = %zd\n", ret, count);
		return -EXIT_FAILURE;
	}

	return 0;
}

int ft_init_av(void)
{
	return ft_init_av_dst_addr(av, ep, remote_fi_addrs);
}

/* TODO: retry send for unreliable endpoints */
int ft_init_av_dst_addr(struct fid_av *av_ptr, struct fid_ep *ep_ptr,
		fi_addr_t *remote_addr)
{
	size_t addrlen;
	int ret, i;
	char* alladdrs = NULL;
	size_t len = 512;
	char epname[512];

	alladdrs = (char*) malloc(rdesc.nodes * rdesc.addrlen);
	assert(alladdrs);
	for(int i=0; i< rdesc.nodes; i++){
		char* ptr = alladdrs + i * rdesc.addrlen;
		ret = rt_get(i, "epname", ptr, rdesc.addrlen);
		if(ret){
			ERR_MSG("Error getting EP address name from %i (%d).", i, ret);
			return -1;
		}
	}

	ret = ft_av_insert(av_ptr, alladdrs, rdesc.nodes, remote_addr, 0, NULL);
	return 0;
}

int ft_gather2(void* sendbuf, size_t size, void* recvbuf, unsigned int n, unsigned int id)
{
	int i, ret;
	void* buf;
	unsigned int* src_id;
	unsigned int* ready;

	if(rdesc.nid == id){
		memcpy(recvbuf + id * size, sendbuf, size);
		DEBUG_MSG("\t Gathering node %d, key: 0x%lx, addr: 0x%lx", 
			  id, ((struct fi_rma_iov*) sendbuf)->key, 
			  ((struct fi_rma_iov*) sendbuf)->addr);
		
		for(i=0; i<n; i++){
			if(i == id)
				continue;
			ready = (unsigned int*)(tx_buf + ft_tx_prefix_size()); 
			ready[0]=123456789;
			ready[1]=987654321;
			DEBUG_MSG("\t Sending ready to node %d: %u %u",i, ready[0],ready[1]);
			ret = ft_tx(ep, remote_fi_addrs[i], sizeof(unsigned int)*2, &tx_ctx);
			if(ret)
				return ret;
			ret = ft_rx(ep, size + sizeof(unsigned int));
			if (ret)
				return ret;

			src_id = (unsigned int*) (rx_buf + ft_rx_prefix_size());
			buf = recvbuf + *src_id * size;
			memcpy(buf, (void*) src_id + sizeof(unsigned int), size);

			DEBUG_MSG("\t %d - Recevied IOV from node %d, key: 0x%lx, addr: 0x%lx", 
				  i, *src_id, ((struct fi_rma_iov*) buf)->key, 
				  ((struct fi_rma_iov*) buf)->addr);
		} 
	} else {
		ret = ft_rx(ep, sizeof(unsigned int)*2);
		if (ret)
			return ret;
		ready = (unsigned int*) (rx_buf + ft_rx_prefix_size());
		DEBUG_MSG("\t Recieved ready from node %d: %u %u",id , ready[0],ready[1]);
		
		if(ready[0] != 123456789 || ready[1] !=987654321 ){
			return -1; //TODO return a better error code...
		}
		DEBUG_MSG("\t Sending IOV to node %d, key: 0x%lx, addr: 0x%lx...", id, 
			  ((struct fi_rma_iov*) sendbuf)->key, 
			  ((struct fi_rma_iov*) sendbuf)->addr);
		src_id = (unsigned int*)(tx_buf + ft_tx_prefix_size()); 
		*src_id = rdesc.nid;
		memcpy((void*)src_id + sizeof(unsigned int), sendbuf, size);
		ret = ft_tx(ep, remote_fi_addrs[id], size + sizeof(unsigned int), &tx_ctx);
		if (ret)
			return ret;		
	}
	return 0;
}

int ft_gather(void* sendbuf, size_t size, void* recvbuf, unsigned int n, unsigned int id)
{
	int i, ret;
	void* buf;
	unsigned int* src_id;

	if(rdesc.nid == id){
		memcpy(recvbuf + id * size, sendbuf, size);
		DEBUG_MSG("\t Gathering node %d, key: 0x%lx, addr: 0x%lx", 
			  id, ((struct fi_rma_iov*) sendbuf)->key, 
			  ((struct fi_rma_iov*) sendbuf)->addr);
		
		for(i=0; i<n; i++){
			if(i == id)
				continue;
			/*
			ret = ft_get_rx_comp(rx_seq);
			if(ret)
				return ret;
			
			ret = ft_post_rx(ep, rx_size, &rx_ctx);
			*/
			ret = ft_rx(ep, size + sizeof(unsigned int));
			if (ret)
				return ret;

			src_id = (unsigned int*) (rx_buf + ft_rx_prefix_size());
			buf = recvbuf + *src_id * size;
			memcpy(buf, (void*) src_id + sizeof(unsigned int), size);

			DEBUG_MSG("\t %d - Recevied IOV from node %d, key: 0x%lx, addr: 0x%lx", 
				  i, *src_id, ((struct fi_rma_iov*) buf)->key, 
				  ((struct fi_rma_iov*) buf)->addr);
				
		}
	}else{
		DEBUG_MSG("\t Sending IOV to node %d, key: 0x%lx, addr: 0x%lx...", id, 
			  ((struct fi_rma_iov*) sendbuf)->key, 
			  ((struct fi_rma_iov*) sendbuf)->addr);
		src_id = (unsigned int*)(tx_buf + ft_tx_prefix_size()); 
		*src_id = rdesc.nid;
		memcpy((void*)src_id + sizeof(unsigned int), sendbuf, size);
		ret = ft_tx(ep, remote_fi_addrs[id], size + sizeof(unsigned int), &tx_ctx);
		if (ret)
			return ret;		
	}
	return 0;
}

int ft_scatter(void* sendbuf, size_t size, void* recvbuf, unsigned int n, unsigned int id)
{
	unsigned int i;
	int ret;

	if(rdesc.nid == id){
		memcpy(tx_buf + ft_tx_prefix_size(), sendbuf, size);
		for(i=1; i< n; i++){
			if(i == id)
				continue;

			ret = ft_tx(ep, remote_fi_addrs[i], size, &tx_ctx);
			if(ret)
				return ret;
			DEBUG_MSG("\t Sent IOVs to node %d", i);
		}		
	}else{
		ret = ft_get_rx_comp(rx_seq);
		if (ret)
			return ret;

		ret = ft_post_rx(ep, rx_size, &rx_ctx);
		memcpy(recvbuf, rx_buf + ft_rx_prefix_size(), size);
		DEBUG_MSG("\t Received IOV from node %d", id);
	}
	return 0;
}


int ft_exchange_keys(struct fi_rma_iov *peer_iov, struct fid_mr* mr, void* addr)
{
	struct fi_rma_iov rma_iov;
	int ret;
	unsigned int id = rdesc.nid;

	if(rt_get_size() == 1)
		return 0;

	if ((fi->domain_attr->mr_mode == FI_MR_BASIC) ||
	    (fi->domain_attr->mr_mode & FI_MR_VIRT_ADDR)) {
		rma_iov.addr = (uintptr_t) addr;
	} else {
		rma_iov.addr = 0;
	}
	rma_iov.key = fi_mr_key(mr);

	DEBUG_MSG("Exchanging Keys (key: 0x%lx, addr: 0x%lx)....",  rma_iov.key, rma_iov.addr);
	ret = ft_gather2(&rma_iov, sizeof(struct fi_rma_iov), peer_iov, rdesc.nodes, 0);
	if(ret)
		return ret;

#ifdef _DEBUG
	if(rdesc.nid == 0)
		for(int i=0; i<rdesc.nodes; i++)
			DEBUG_MSG("IOV[%d]: \t\t key 0x%lx addr 0x%lx", i, peer_iov[i].key, peer_iov[i].addr);
#endif
	
	ret = ft_scatter(peer_iov, sizeof(struct fi_rma_iov) * rdesc.nodes, peer_iov, rdesc.nodes, 0);
	if(ret)
		return ret;

#ifdef _DEBUG
	if(rdesc.nid)
		for(int i=0; i<rdesc.nodes; i++)
			DEBUG_MSG("IOV[%d]: \t\t key 0x%lx addr 0x%lx", i, peer_iov[i].key, peer_iov[i].addr);
#endif

	return ret;
}

static void ft_cleanup_mr_array(struct ft_context *ctx_arr, char **mr_bufs)
{
	int i;

	if (!mr_bufs)
		return;

	for (i = 0; i < opts.window_size; i++) {
		FT_CLOSE_FID(ctx_arr[i].mr);
		free(mr_bufs[i]);
	}
}

static void ft_close_fids(void)
{
	if (mr != &no_mr)
		FT_CLOSE_FID(mr);
	FT_CLOSE_FID(mc);
	FT_CLOSE_FID(alias_ep);
	FT_CLOSE_FID(ep);
	FT_CLOSE_FID(pep);
	if (opts.options & FT_OPT_CQ_SHARED) {
		FT_CLOSE_FID(txcq);
	} else {
		FT_CLOSE_FID(rxcq);
		FT_CLOSE_FID(txcq);
	}
	FT_CLOSE_FID(rxcntr);
	FT_CLOSE_FID(txcntr);
	FT_CLOSE_FID(pollset);
	FT_CLOSE_FID(av);
	FT_CLOSE_FID(eq);
	FT_CLOSE_FID(domain);
	FT_CLOSE_FID(waitset);
	FT_CLOSE_FID(fabric);
}

void ft_free_res(void)
{
	ft_cleanup_mr_array(tx_ctx_arr, tx_mr_bufs);
	ft_cleanup_mr_array(rx_ctx_arr, rx_mr_bufs);

	free(tx_ctx_arr);
	free(rx_ctx_arr);
	tx_ctx_arr = NULL;
	rx_ctx_arr = NULL;

	ft_close_fids();

	if (buf) {
		free(buf);
		buf = rx_buf = tx_buf = NULL;
		buf_size = rx_size = tx_size = tx_mr_size = rx_mr_size = 0;
	}
	if (fi_pep) {
		fi_freeinfo(fi_pep);
		fi_pep = NULL;
	}
	if (fi) {
		fi_freeinfo(fi);
		fi = NULL;
	}
	if (hints) {
		fi_freeinfo(hints);
		hints = NULL;
	}
}

static int dupaddr(void **dst_addr, size_t *dst_addrlen,
		void *src_addr, size_t src_addrlen)
{
	*dst_addr = malloc(src_addrlen);
	if (!*dst_addr) {
		FT_ERR("address allocation failed");
		return EAI_MEMORY;
	}
	*dst_addrlen = src_addrlen;
	memcpy(*dst_addr, src_addr, src_addrlen);
	return 0;
}


char *size_str(char str[FT_STR_LEN], long long size)
{
	long long base, fraction = 0;
	char mag;

	memset(str, '\0', FT_STR_LEN);

	if (size >= (1 << 30)) {
		base = 1 << 30;
		mag = 'g';
	} else if (size >= (1 << 20)) {
		base = 1 << 20;
		mag = 'm';
	} else if (size >= (1 << 10)) {
		base = 1 << 10;
		mag = 'k';
	} else {
		base = 1;
		mag = '\0';
	}

	if (size / base < 10)
		fraction = (size % base) * 10 / base;

	if (fraction)
		snprintf(str, FT_STR_LEN, "%lld.%lld%c", size / base, fraction, mag);
	else
		snprintf(str, FT_STR_LEN, "%lld%c", size / base, mag);

	return str;
}

char *cnt_str(char str[FT_STR_LEN], long long cnt)
{
	if (cnt >= 1000000000)
		snprintf(str, FT_STR_LEN, "%lldb", cnt / 1000000000);
	else if (cnt >= 1000000)
		snprintf(str, FT_STR_LEN, "%lldm", cnt / 1000000);
	else if (cnt >= 1000)
		snprintf(str, FT_STR_LEN, "%lldk", cnt / 1000);
	else
		snprintf(str, FT_STR_LEN, "%lld", cnt);

	return str;
}

int size_to_count(int size)
{
	if (size >= (1 << 20))
		return (opts.options & FT_OPT_BW) ? 200 : 100;
	else if (size >= (1 << 16))
		return (opts.options & FT_OPT_BW) ? 2000 : 1000;
	else
		return (opts.options & FT_OPT_BW) ? 20000: 10000;
}

static const size_t datatype_size_table[] = {
	[FI_INT8]   = sizeof(int8_t),
	[FI_UINT8]  = sizeof(uint8_t),
	[FI_INT16]  = sizeof(int16_t),
	[FI_UINT16] = sizeof(uint16_t),
	[FI_INT32]  = sizeof(int32_t),
	[FI_UINT32] = sizeof(uint32_t),
	[FI_INT64]  = sizeof(int64_t),
	[FI_UINT64] = sizeof(uint64_t),
	[FI_FLOAT]  = sizeof(float),
	[FI_DOUBLE] = sizeof(double),
	[FI_FLOAT_COMPLEX]  = sizeof(OFI_COMPLEX(float)),
	[FI_DOUBLE_COMPLEX] = sizeof(OFI_COMPLEX(double)),
	[FI_LONG_DOUBLE]    = sizeof(long double),
	[FI_LONG_DOUBLE_COMPLEX] = sizeof(OFI_COMPLEX(long_double)),
};

size_t datatype_to_size(enum fi_datatype datatype)
{
	if (datatype >= FI_DATATYPE_LAST)
		return 0;

	return datatype_size_table[datatype];
}

void init_test(struct ft_opts *opts, char *test_name, size_t test_name_len)
{
	char sstr[FT_STR_LEN];

	size_str(sstr, opts->transfer_size);
	if (!strcmp(test_name, "custom"))
		snprintf(test_name, test_name_len, "%s_lat", sstr);
	if (!(opts->options & FT_OPT_ITER))
		opts->iterations = size_to_count(opts->transfer_size);
}

static int ft_progress(struct fid_cq *cq, uint64_t total, uint64_t *cq_cntr)
{
	struct fi_cq_err_entry comp;
	int ret;

	ret = fi_cq_read(cq, &comp, 1);
	if (ret > 0)
		(*cq_cntr)++;

	if (ret >= 0 || ret == -FI_EAGAIN)
		return 0;

	if (ret == -FI_EAVAIL) {
		ret = ft_cq_readerr(cq);
		(*cq_cntr)++;
	} else {
		FT_PRINTERR("fi_cq_read/sread", ret);
	}
	return ret;
}

#define FT_POST(post_fn, progress_fn, cq, seq, cq_cntr, op_str, ...)		\
	do {									\
		int timeout_save;						\
		int ret, rc;							\
										\
		while (1) {							\
			ret = post_fn(__VA_ARGS__);				\
			if (!ret)						\
				break;						\
										\
			if (ret != -FI_EAGAIN) {				\
				FT_PRINTERR(op_str, ret);			\
				return ret;					\
			}							\
										\
			timeout_save = timeout;					\
			timeout = 0;						\
			rc = progress_fn(cq, seq, cq_cntr);			\
			if (rc && rc != -FI_EAGAIN) {				\
				FT_ERR("Failed to get " op_str " completion");	\
				return rc;					\
			}							\
			timeout = timeout_save;					\
		}								\
		seq++;								\
	} while (0)

ssize_t ft_post_tx_buf(struct fid_ep *ep, fi_addr_t fi_addr, size_t size,
		       uint64_t data, void *ctx,
		       void *op_buf, void *op_mr_desc, uint64_t op_tag)
{
	size += ft_tx_prefix_size();
	if (hints->caps & FI_TAGGED) {
		op_tag = op_tag ? op_tag : tx_seq;
		if (data != NO_CQ_DATA) {
			FT_POST(fi_tsenddata, ft_progress, txcq, tx_seq,
				&tx_cq_cntr, "transmit", ep, op_buf, size,
				op_mr_desc, data, fi_addr, op_tag, ctx);
		} else {
			FT_POST(fi_tsend, ft_progress, txcq, tx_seq,
				&tx_cq_cntr, "transmit", ep, op_buf, size,
				op_mr_desc, fi_addr, op_tag, ctx);
		}
	} else {
		if (data != NO_CQ_DATA) {
			FT_POST(fi_senddata, ft_progress, txcq, tx_seq,
				&tx_cq_cntr, "transmit", ep, op_buf, size,
				op_mr_desc, data, fi_addr, ctx);

		} else {
			FT_POST(fi_send, ft_progress, txcq, tx_seq,
				&tx_cq_cntr, "transmit", ep, op_buf, size,
				op_mr_desc, fi_addr, ctx);
		}
	}
	return 0;
}

ssize_t ft_post_tx(struct fid_ep *ep, fi_addr_t fi_addr, size_t size,
		   uint64_t data, void *ctx)
{
	return ft_post_tx_buf(ep, fi_addr, size, data,
			      ctx, tx_buf, mr_desc, ft_tag);
}

ssize_t ft_tx(struct fid_ep *ep, fi_addr_t fi_addr, size_t size, void *ctx)
{
	ssize_t ret;

	if (ft_check_opts(FT_OPT_VERIFY_DATA | FT_OPT_ACTIVE))
		ft_fill_buf((char *) tx_buf + ft_tx_prefix_size(), size);

	ret = ft_post_tx(ep, fi_addr, size, NO_CQ_DATA, ctx);
	if (ret)
		return ret;

	ret = ft_get_tx_comp(tx_seq);
	return ret;
}

ssize_t ft_post_inject(struct fid_ep *ep, fi_addr_t fi_addr, size_t size)
{
	if (hints->caps & FI_TAGGED) {
		FT_POST(fi_tinject, ft_progress, txcq, tx_seq, &tx_cq_cntr,
			"inject", ep, tx_buf, size + ft_tx_prefix_size(),
			fi_addr, tx_seq);
	} else {
		FT_POST(fi_inject, ft_progress, txcq, tx_seq, &tx_cq_cntr,
			"inject", ep, tx_buf, size + ft_tx_prefix_size(),
			fi_addr);
	}

	tx_cq_cntr++;
	return 0;
}

ssize_t ft_inject(struct fid_ep *ep, fi_addr_t fi_addr, size_t size)
{
	ssize_t ret;

	if (ft_check_opts(FT_OPT_VERIFY_DATA | FT_OPT_ACTIVE))
		ft_fill_buf((char *) tx_buf + ft_tx_prefix_size(), size);

	ret = ft_post_inject(ep, fi_addr, size);
	if (ret)
		return ret;

	return ret;
}

ssize_t ft_post_rma(enum ft_rma_opcodes op, struct fid_ep *ep, size_t size,
		    struct fi_rma_iov *remote, void* src, unsigned long id, void* desc, void *context)
{
	switch (op) {
	case FT_RMA_WRITE:
		FT_POST(fi_write, ft_progress, txcq, tx_seq, &tx_cq_cntr,
			"fi_write", ep, src, size, desc,
			remote_fi_addrs[id], remote->addr, remote->key, context);
		break;
	case FT_RMA_WRITEDATA:
		FT_POST(fi_writedata, ft_progress, txcq, tx_seq, &tx_cq_cntr,
			"fi_writedata", ep, src, size, desc,
			remote_cq_data, remote_fi_addrs[id], remote->addr,
			remote->key, context);
		break;
	case FT_RMA_READ:
		FT_POST(fi_read, ft_progress, txcq, tx_seq, &tx_cq_cntr,
			"fi_read", ep, src, size, desc,
			remote_fi_addrs[id], remote->addr, remote->key, context);
		break;
	default:
		FT_ERR("Unknown RMA op type\n");
		return EXIT_FAILURE;
	}

	return 0;
}

#if 0
ssize_t ft_rma(enum ft_rma_opcodes op, struct fid_ep *ep, size_t size,
		struct fi_rma_iov *remote, void *context)
{
	int ret;

	ret = ft_post_rma(op, ep, size, remote, context);
	if (ret)
		return ret;

	if (op == FT_RMA_WRITEDATA) {
		if (fi->rx_attr->mode & FI_RX_CQ_DATA) {
			ret = ft_rx(ep, 0);
		} else {
			ret = ft_get_rx_comp(rx_seq);
			/* Just increment the seq # instead of posting recv so
			 * that we wait for remote write completion on the next
			 * iteration. */
			rx_seq++;
		}
		if (ret)
			return ret;
	}

	ret = ft_get_tx_comp(tx_seq);
	if (ret)
		return ret;

	return 0;
}
#endif 

ssize_t ft_post_rma_inject(enum ft_rma_opcodes op, struct fid_ep *ep, size_t size,
			   struct fi_rma_iov *remote, const void* src, unsigned int id)
{
	switch (op) {
	case FT_RMA_WRITE:
		DEBUG_MSG("\t addr: 0x%lx key: 0x%lx", remote->addr, remote->key);
		FT_POST(fi_inject_write, ft_progress, txcq, tx_seq, &tx_cq_cntr,
			"fi_inject_write", ep, src, size,
			remote_fi_addrs[id], remote->addr, remote->key);
		break;
	case FT_RMA_WRITEDATA:
		FT_POST(fi_inject_writedata, ft_progress, txcq, tx_seq,
			&tx_cq_cntr, "fi_inject_writedata", ep, tx_buf,
			opts.transfer_size, remote_cq_data, remote_fi_addrs[id],
			remote->addr, remote->key);
		break;
	default:
		FT_ERR("Unknown RMA inject op type\n");
		return EXIT_FAILURE;
	}

	tx_cq_cntr++;
	return 0;
}

ssize_t ft_post_atomic(enum ft_atomic_opcodes opcode, struct fid_ep *ep,
		       void *compare, void *compare_desc, void *result,
		       void *result_desc, struct fi_rma_iov *remote,
		       enum fi_datatype datatype, enum fi_op atomic_op,
		       void *context)
{
	size_t size, count;

	size = datatype_to_size(datatype);
	if (!size) {
		FT_ERR("Unknown datatype\n");
		return EXIT_FAILURE;
	}
	count = opts.transfer_size / size;

	switch (opcode) {
	case FT_ATOMIC_BASE:
		FT_POST(fi_atomic, ft_progress, txcq, tx_seq, &tx_cq_cntr,
			"fi_atomic", ep, buf, count, mr_desc, remote_fi_addrs[(rt_get_rank() + 1) % rt_get_size()],
			remote->addr, remote->key, datatype, atomic_op, context);
		break;
	case FT_ATOMIC_FETCH:
		FT_POST(fi_fetch_atomic, ft_progress, txcq, tx_seq, &tx_cq_cntr,
			"fi_fetch_atomic", ep, buf, count, mr_desc, result,
			result_desc, remote_fi_addrs[(rt_get_rank() + 1) % rt_get_size()], remote->addr, remote->key,
			datatype, atomic_op, context);
		break;
	case FT_ATOMIC_COMPARE:
		FT_POST(fi_compare_atomic, ft_progress, txcq, tx_seq,
			&tx_cq_cntr, "fi_compare_atomic", ep, buf, count,
			mr_desc, compare, compare_desc, result, result_desc,
			remote_fi_addrs[(rt_get_rank() + 1) % rt_get_size()], remote->addr, remote->key, datatype,
			atomic_op, context);
		break;
	default:
		FT_ERR("Unknown atomic opcode\n");
		return EXIT_FAILURE;
	}

	return 0;
}

static int check_atomic_attr(enum fi_op op, enum fi_datatype datatype,
			     uint64_t flags)
{
	struct fi_atomic_attr attr;
	int ret;

	ret = fi_query_atomic(domain, datatype, op, &attr, flags);
	if (ret) {
		FT_PRINTERR("fi_query_atomic", ret);
		return ret;
	}

	if (attr.size != datatype_to_size(datatype)) {
		fprintf(stderr, "Provider atomic size mismatch\n");
		return -FI_ENOSYS;
	}

	return 0;
}

int check_base_atomic_op(struct fid_ep *endpoint, enum fi_op op,
			 enum fi_datatype datatype, size_t *count)
{
	int ret;

	ret = fi_atomicvalid(endpoint, datatype, op, count);
	if (ret)
		return ret;

	return check_atomic_attr(op, datatype, 0);
}

int check_fetch_atomic_op(struct fid_ep *endpoint, enum fi_op op,
			  enum fi_datatype datatype, size_t *count)
{
	int ret;

	ret = fi_fetch_atomicvalid(endpoint, datatype, op, count);
	if (ret)
		return ret;

	return check_atomic_attr(op, datatype, FI_FETCH_ATOMIC);
}

int check_compare_atomic_op(struct fid_ep *endpoint, enum fi_op op,
			    enum fi_datatype datatype, size_t *count)
{
	int ret;

	ret = fi_compare_atomicvalid(endpoint, datatype, op, count);
	if (ret)
		return ret;

	return check_atomic_attr(op, datatype, FI_COMPARE_ATOMIC);
}

ssize_t ft_post_rx_buf(struct fid_ep *ep, size_t size, void *ctx,
		       void *op_buf, void *op_mr_desc, uint64_t op_tag)
{
	size = MAX(size, FT_MAX_CTRL_MSG) + ft_rx_prefix_size();
	if (hints->caps & FI_TAGGED) {
		op_tag = op_tag ? op_tag : rx_seq;
		FT_POST(fi_trecv, ft_progress, rxcq, rx_seq, &rx_cq_cntr,
			"receive", ep, op_buf, size, op_mr_desc, 0, op_tag,
			0, ctx);
	} else {
		FT_POST(fi_recv, ft_progress, rxcq, rx_seq, &rx_cq_cntr,
			"receive", ep, op_buf, size, op_mr_desc, 0, ctx);
	}
	return 0;
}

ssize_t ft_post_rx(struct fid_ep *ep, size_t size, void *ctx)
{
	return ft_post_rx_buf(ep, size, ctx, rx_buf, mr_desc, ft_tag);
}

ssize_t ft_rx(struct fid_ep *ep, size_t size)
{
	ssize_t ret;

	ret = ft_get_rx_comp(rx_seq);
	if (ret)
		return ret;

	if (ft_check_opts(FT_OPT_VERIFY_DATA | FT_OPT_ACTIVE)) {
		ret = ft_check_buf((char *) rx_buf + ft_rx_prefix_size(), size);
		if (ret)
			return ret;
	}
	/* TODO: verify CQ data, if available */

	/* Ignore the size arg. Post a buffer large enough to handle all message
	 * sizes. ft_sync() makes use of ft_rx() and gets called in tests just before
	 * message size is updated. The recvs posted are always for the next incoming
	 * message */
	ret = ft_post_rx(ep, rx_size, &rx_ctx);
	return ret;
}

/*
 * Received messages match tagged buffers in order, but the completions can be
 * reported out of order.  A tag is valid if it's within the current window.
 */
static inline int
ft_tag_is_valid(struct fid_cq * cq, struct fi_cq_err_entry *comp, uint64_t tag)
{
	int valid = 1;

	if ((hints->caps & FI_TAGGED) && (cq == rxcq)) {
		if (opts.options & FT_OPT_BW) {
			/* valid: (tag - window) < comp->tag < (tag + window) */
			valid = (tag < comp->tag + opts.window_size) &&
				(comp->tag < tag + opts.window_size);
		} else {
			valid = (comp->tag == tag);
		}

		if (!valid) {
			FT_ERR("Tag mismatch!. Expected: %"PRIu64", actual: %"
				PRIu64, tag, comp->tag);
		}
	}

	return valid;
}
/*
 * fi_cq_err_entry can be cast to any CQ entry format.
 */
static int ft_spin_for_comp(struct fid_cq *cq, uint64_t *cur,
			    uint64_t total, int timeout)
{
	struct fi_cq_err_entry comp;
	struct timespec a, b;
	int ret;

	if (timeout >= 0)
		clock_gettime(CLOCK_MONOTONIC, &a);

	do {
		ret = fi_cq_read(cq, &comp, 1);
		if (ret > 0) {
			if (timeout >= 0)
				clock_gettime(CLOCK_MONOTONIC, &a);
			if (!ft_tag_is_valid(cq, &comp, ft_tag ? ft_tag : rx_cq_cntr))
				return -FI_EOTHER;
			(*cur)++;
		} else if (ret < 0 && ret != -FI_EAGAIN) {
			return ret;
		} else if (timeout >= 0) {
			clock_gettime(CLOCK_MONOTONIC, &b);
			if ((b.tv_sec - a.tv_sec) > timeout) {
				fprintf(stderr, "%ds timeout expired\n", timeout);
				return -FI_ENODATA;
			}
		}
	} while (total - *cur > 0);

	return 0;
}

/*
 * fi_cq_err_entry can be cast to any CQ entry format.
 */
static int ft_wait_for_comp(struct fid_cq *cq, uint64_t *cur,
			    uint64_t total, int timeout)
{
	struct fi_cq_err_entry comp;
	int ret;

	while (total - *cur > 0) {
		ret = fi_cq_sread(cq, &comp, 1, NULL, timeout);
		if (ret > 0) {
			if (!ft_tag_is_valid(cq, &comp, ft_tag ? ft_tag : rx_cq_cntr))
				return -FI_EOTHER;
			(*cur)++;
		} else if (ret < 0 && ret != -FI_EAGAIN) {
			return ret;
		}
	}

	return 0;
}

/*
 * fi_cq_err_entry can be cast to any CQ entry format.
 */
static int ft_fdwait_for_comp(struct fid_cq *cq, uint64_t *cur,
			    uint64_t total, int timeout)
{
	struct fi_cq_err_entry comp;
	struct fid *fids[1];
	int fd, ret;

	fd = cq == txcq ? tx_fd : rx_fd;
	fids[0] = &cq->fid;

	while (total - *cur > 0) {
		ret = fi_trywait(fabric, fids, 1);
		if (ret == FI_SUCCESS) {
			ret = ft_poll_fd(fd, timeout);
			if (ret && ret != -FI_EAGAIN)
				return ret;
		}

		ret = fi_cq_read(cq, &comp, 1);
		if (ret > 0) {
			if (!ft_tag_is_valid(cq, &comp, ft_tag ? ft_tag : rx_cq_cntr))
				return -FI_EOTHER;
			(*cur)++;
		} else if (ret < 0 && ret != -FI_EAGAIN) {
			return ret;
		}
	}

	return 0;
}

static int ft_get_cq_comp(struct fid_cq *cq, uint64_t *cur,
			  uint64_t total, int timeout)
{
	int ret;

	switch (opts.comp_method) {
	case FT_COMP_SREAD:
	case FT_COMP_YIELD:
		ret = ft_wait_for_comp(cq, cur, total, timeout);
		break;
	case FT_COMP_WAIT_FD:
		ret = ft_fdwait_for_comp(cq, cur, total, timeout);
		break;
	default:
		ret = ft_spin_for_comp(cq, cur, total, timeout);
		break;
	}

	if (ret) {
		if (ret == -FI_EAVAIL) {
			ret = ft_cq_readerr(cq);
			(*cur)++;
		} else {
			FT_PRINTERR("ft_get_cq_comp", ret);
		}
	}
	return ret;
}

static int ft_spin_for_cntr(struct fid_cntr *cntr, uint64_t total, int timeout)
{
	struct timespec a, b;
	uint64_t cur;

	if (timeout >= 0)
		clock_gettime(CLOCK_MONOTONIC, &a);

	for (;;) {
		cur = fi_cntr_read(cntr);
		if (cur >= total)
			return 0;

		if (timeout >= 0) {
			clock_gettime(CLOCK_MONOTONIC, &b);
			if ((b.tv_sec - a.tv_sec) > timeout)
				break;
		}
	}

	fprintf(stderr, "%ds timeout expired\n", timeout);
	return -FI_ENODATA;
}

static int ft_wait_for_cntr(struct fid_cntr *cntr, uint64_t total, int timeout)
{
	int ret;

	while (fi_cntr_read(cntr) < total) {
		ret = fi_cntr_wait(cntr, total, timeout);
		if (ret)
			FT_PRINTERR("fi_cntr_wait", ret);
		else
			break;
	}
	return 0;
}

static int ft_get_cntr_comp(struct fid_cntr *cntr, uint64_t total, int timeout)
{
	int ret = 0;

	switch (opts.comp_method) {
	case FT_COMP_SREAD:
	case FT_COMP_WAITSET:
	case FT_COMP_WAIT_FD:
	case FT_COMP_YIELD:
		ret = ft_wait_for_cntr(cntr, total, timeout);
		break;
	default:
		ret = ft_spin_for_cntr(cntr, total, timeout);
		break;
	}

	if (ret)
		FT_PRINTERR("fs_get_cntr_comp", ret);

	return ret;
}

int ft_get_rx_comp(uint64_t total)
{
	int ret = FI_SUCCESS;

	if (opts.options & FT_OPT_RX_CQ) {
		ret = ft_get_cq_comp(rxcq, &rx_cq_cntr, total, timeout);
	} else if (rxcntr) {
		ret = ft_get_cntr_comp(rxcntr, total, timeout);
	} else {
		FT_ERR("Trying to get a RX completion when no RX CQ or counter were opened");
		ret = -FI_EOTHER;
	}
	return ret;
}

int ft_get_tx_comp(uint64_t total)
{
	int ret;

	if (opts.options & FT_OPT_TX_CQ) {
		ret = ft_get_cq_comp(txcq, &tx_cq_cntr, total, -1);
	} else if (txcntr) {
		ret = ft_get_cntr_comp(txcntr, total, -1);
	} else {
		FT_ERR("Trying to get a TX completion when no TX CQ or counter were opened");
		ret = -FI_EOTHER;
	}
	return ret;
}

int ft_sendmsg(struct fid_ep *ep, fi_addr_t fi_addr,
		size_t size, void *ctx, int flags)
{
	int ret;
	struct fi_msg msg;
	struct fi_msg_tagged tagged_msg;
	struct iovec msg_iov;

	msg_iov.iov_base = tx_buf;
	msg_iov.iov_len = size;

	if (hints->caps & FI_TAGGED) {
		tagged_msg.msg_iov = &msg_iov;
		tagged_msg.desc = &mr_desc;
		tagged_msg.iov_count = 1;
		tagged_msg.addr = fi_addr;
		tagged_msg.data = NO_CQ_DATA;
		tagged_msg.context = ctx;
		tagged_msg.tag = ft_tag ? ft_tag : tx_seq;
		tagged_msg.ignore = 0;

		ret = fi_tsendmsg(ep, &tagged_msg, flags);
		if (ret) {
			FT_PRINTERR("fi_tsendmsg", ret);
			return ret;
		}
	} else {
		msg.msg_iov = &msg_iov;
		msg.desc = &mr_desc;
		msg.iov_count = 1;
		msg.addr = fi_addr;
		msg.data = NO_CQ_DATA;
		msg.context = ctx;

		ret = fi_sendmsg(ep, &msg, flags);
		if (ret) {
			FT_PRINTERR("fi_sendmsg", ret);
			return ret;
		}
	}

	return 0;
}

int ft_recvmsg(struct fid_ep *ep, fi_addr_t fi_addr,
	       size_t size, void *ctx, int flags)
{
	int ret;
	struct fi_msg msg;
	struct fi_msg_tagged tagged_msg;
	struct iovec msg_iov;

	msg_iov.iov_base = rx_buf;
	msg_iov.iov_len = size;

	if (hints->caps & FI_TAGGED) {
		tagged_msg.msg_iov = &msg_iov;
		tagged_msg.desc = &mr_desc;
		tagged_msg.iov_count = 1;
		tagged_msg.addr = fi_addr;
		tagged_msg.data = NO_CQ_DATA;
		tagged_msg.context = ctx;
		tagged_msg.tag = ft_tag ? ft_tag : tx_seq;
		tagged_msg.ignore = 0;

		ret = fi_trecvmsg(ep, &tagged_msg, flags);
		if (ret) {
			FT_PRINTERR("fi_trecvmsg", ret);
			return ret;
		}
	} else {
		msg.msg_iov = &msg_iov;
		msg.desc = &mr_desc;
		msg.iov_count = 1;
		msg.addr = fi_addr;
		msg.data = NO_CQ_DATA;
		msg.context = ctx;

		ret = fi_recvmsg(ep, &msg, flags);
		if (ret) {
			FT_PRINTERR("fi_recvmsg", ret);
			return ret;
		}
	}

	return 0;
}

int ft_cq_read_verify(struct fid_cq *cq, void *op_context)
{
	int ret;
	struct fi_cq_err_entry completion;

	do {
		/* read events from the completion queue */
		ret = fi_cq_read(cq, (void *)&completion, 1);

		if (ret > 0) {
			if (op_context != completion.op_context) {
				fprintf(stderr, "ERROR: op ctx=%p cq_ctx=%p\n",
					op_context, completion.op_context);
				return -FI_EOTHER;
			}
			if (!ft_tag_is_valid(cq, &completion,
					     ft_tag ? ft_tag : rx_cq_cntr))
				return -FI_EOTHER;
		} else if ((ret <= 0) && (ret != -FI_EAGAIN)) {
			FT_PRINTERR("POLL: Error\n", ret);
			if (ret == -FI_EAVAIL)
				FT_PRINTERR("POLL: error available\n", ret);
			return -FI_EOTHER;
		}
	} while (ret == -FI_EAGAIN);

	return 0;
}

int ft_cq_readerr(struct fid_cq *cq)
{
	struct fi_cq_err_entry cq_err;
	int ret;

	memset(&cq_err, 0, sizeof(cq_err));
	ret = fi_cq_readerr(cq, &cq_err, 0);
	if (ret < 0) {
		FT_PRINTERR("fi_cq_readerr", ret);
	} else {
		FT_CQ_ERR(cq, cq_err, NULL, 0);
		ret = -cq_err.err;
	}
	return ret;
}

void eq_readerr(struct fid_eq *eq, const char *eq_str)
{
	struct fi_eq_err_entry eq_err;
	int rd;

	memset(&eq_err, 0, sizeof(eq_err));
	rd = fi_eq_readerr(eq, &eq_err, 0);
	if (rd != sizeof(eq_err)) {
		FT_PRINTERR("fi_eq_readerr", rd);
	} else {
		FT_EQ_ERR(eq, eq_err, NULL, 0);
	}
}

int ft_sync()
{
	char buf;
	int ret;

	DEBUG_MSG("....");
	if (rt_get_rank()) {
		if (!(opts.options & FT_OPT_OOB_SYNC)) {
			ret = ft_tx(ep, remote_fi_addrs[0], 1, &tx_ctx);
			if (ret)
				return ret;

			ret = ft_rx(ep, 1);
		} else {
			ret = ft_sock_send(oob_sock, &buf, 1);
			if (ret)
				return ret;

			ret = ft_sock_recv(oob_sock, &buf, 1);
			if (ret)
				return ret;
		}
	} else {
		if (!(opts.options & FT_OPT_OOB_SYNC)) {
			ret = ft_rx(ep, 1);
			if (ret)
				return ret;

			ret = ft_tx(ep, remote_fi_addrs[1], 1, &tx_ctx);
		} else {
			ret = ft_sock_recv(oob_sock, &buf, 1);
			if (ret)
				return ret;

			ret = ft_sock_send(oob_sock, &buf, 1);
			if (ret)
				return ret;
		}
	}

	return ret;
}

int ft_sync_pair(int status)
{
	int ret;
	int pair_status;

	if (ft_parent_proc) {
		ret = write(ft_socket_pair[1], &status, sizeof(int));
		if (ret < 0) {
			FT_PRINTERR("write", errno);
			return ret;
		}
		ret = read(ft_socket_pair[1], &pair_status, sizeof(int));
		if (ret < 0) {
			FT_PRINTERR("read", errno);
			return ret;
		}
	} else {
		ret = read(ft_socket_pair[0], &pair_status, sizeof(int));
		if (ret < 0) {
			FT_PRINTERR("read", errno);
			return ret;
		}
		ret = write(ft_socket_pair[0], &status, sizeof(int));
		if (ret < 0) {
			FT_PRINTERR("write", errno);
			return ret;
		}
	}

	/* check status reported the other guy */
	if (pair_status != FI_SUCCESS)
		return pair_status;

	return 0;
}

int ft_fork_and_pair(void)
{
	int ret;

	ret = socketpair(AF_LOCAL, SOCK_STREAM, 0, ft_socket_pair);
	if (ret) {
		FT_PRINTERR("socketpair", errno);
		return -errno;
	}

	ft_child_pid = fork();
	if (ft_child_pid < 0) {
		FT_PRINTERR("fork", ft_child_pid);
		return -errno;
	}
	if (ft_child_pid)
		ft_parent_proc = 1;

	return 0;
}

int ft_wait_child(void)
{
	int ret;

	ret = close(ft_socket_pair[0]);
	if (ret) {
		FT_PRINTERR("close", errno);
		return ret;
	}
	ret = close(ft_socket_pair[1]);
	if (ret) {
		FT_PRINTERR("close", errno);
		return ret;
	}
	if (ft_parent_proc) {
		ret = waitpid(ft_child_pid, NULL, WCONTINUED);
		if (ret < 0) {
			FT_PRINTERR("waitpid", errno);
			return ret;
		}
	}

	return 0;
}

int ft_finalize_ep(struct fid_ep *ep)
{
	struct iovec iov;
	int ret;
	struct fi_context ctx;

	strcpy(tx_buf + ft_tx_prefix_size(), "fin");
	iov.iov_base = tx_buf;
	iov.iov_len = 4 + ft_tx_prefix_size();

	if (hints->caps & FI_TAGGED) {
		struct fi_msg_tagged tmsg;

		memset(&tmsg, 0, sizeof tmsg);
		tmsg.msg_iov = &iov;
		tmsg.desc = &mr_desc;
		tmsg.iov_count = 1;
		tmsg.addr = remote_fi_addrs[(rt_get_rank() + 1) % rt_get_size()];
		tmsg.tag = tx_seq;
		tmsg.ignore = 0;
		tmsg.context = &ctx;

		FT_POST(fi_tsendmsg, ft_progress, txcq, tx_seq,
			&tx_cq_cntr, "tsendmsg", ep, &tmsg,
			FI_INJECT | FI_TRANSMIT_COMPLETE);
	} else {
		struct fi_msg msg;

		memset(&msg, 0, sizeof msg);
		msg.msg_iov = &iov;
		msg.desc = &mr_desc;
		msg.iov_count = 1;
		msg.addr = remote_fi_addrs[(rt_get_rank() + 1) % rt_get_size()];
		msg.context = &ctx;

		FT_POST(fi_sendmsg, ft_progress, txcq, tx_seq,
			&tx_cq_cntr, "sendmsg", ep, &msg,
			FI_INJECT | FI_TRANSMIT_COMPLETE);
	}

	ret = ft_get_tx_comp(tx_seq);
	if (ret)
		return ret;

	ret = ft_get_rx_comp(rx_seq);
	if (ret)
		return ret;

	return 0;
}

int ft_finalize(void)
{
	int ret;

	if (fi->domain_attr->mr_mode & FI_MR_RAW) {
		ret = fi_mr_unmap_key(domain, remote_iov[(rdesc.nid + 1) % rdesc.nodes].key);
		if (ret)
			return ret;
	}

	return ft_finalize_ep(ep);
}

int64_t get_elapsed(const struct timespec *b, const struct timespec *a,
		    enum precision p)
{
	int64_t elapsed;

	elapsed = difftime(a->tv_sec, b->tv_sec) * 1000 * 1000 * 1000;
	elapsed += a->tv_nsec - b->tv_nsec;
	return elapsed / p;
}

void show_perf(char *name, size_t tsize, int iters, struct timespec *start,
		struct timespec *end, int xfers_per_iter)
{
	static int header = 1;
	char str[FT_STR_LEN];
	int64_t elapsed = get_elapsed(start, end, MICRO);
	long long bytes = (long long) iters * tsize * xfers_per_iter;
	float usec_per_xfer;

	if (name) {
		if (header) {
			printf("%-50s%-8s%-8s%-8s%8s %10s%13s%13s\n",
					"name", "bytes", "iters",
					"total", "time", "MB/sec",
					"usec/xfer", "Mxfers/sec");
			header = 0;
		}

		printf("%-50s", name);
	} else {
		if (header) {
			printf("%-8s%-8s%-8s%8s %10s%13s%13s\n",
					"bytes", "iters", "total",
					"time", "MB/sec", "usec/xfer",
					"Mxfers/sec");
			header = 0;
		}
	}

	printf("%-8s", size_str(str, tsize));

	printf("%-8s", cnt_str(str, iters));

	printf("%-8s", size_str(str, bytes));

	usec_per_xfer = ((float)elapsed / iters / xfers_per_iter);
	printf("%8.2fs%10.2f%11.2f%11.2f\n",
		elapsed / 1000000.0, bytes / (1.0 * elapsed),
		usec_per_xfer, 1.0/usec_per_xfer);
}

void show_perf_mr(size_t tsize, int iters, struct timespec *start,
		  struct timespec *end, int xfers_per_iter, int argc, char *argv[])
{
	static int header = 1;
	int64_t elapsed = get_elapsed(start, end, MICRO);
	long long total = (long long) iters * tsize * xfers_per_iter;
	int i;
	float usec_per_xfer;

	if (header) {
		printf("---\n");

		for (i = 0; i < argc; ++i)
			printf("%s ", argv[i]);

		printf(":\n");
		header = 0;
	}

	usec_per_xfer = ((float)elapsed / iters / xfers_per_iter);

	printf("- { ");
	printf("xfer_size: %zu, ", tsize);
	printf("iterations: %d, ", iters);
	printf("total: %lld, ", total);
	printf("time: %f, ", elapsed / 1000000.0);
	printf("MB/sec: %f, ", (total) / (1.0 * elapsed));
	printf("usec/xfer: %f, ", usec_per_xfer);
	printf("Mxfers/sec: %f", 1.0/usec_per_xfer);
	printf(" }\n");
}

void ft_fill_buf(void *buf, int size)
{
	char *msg_buf;
	int msg_index;
	static unsigned int iter = 0;
	int i;

	msg_index = ((iter++)*INTEG_SEED) % integ_alphabet_length;
	msg_buf = (char *)buf;
	for (i = 0; i < size; i++) {
		msg_buf[i] = integ_alphabet[msg_index++];
		if (msg_index >= integ_alphabet_length)
			msg_index = 0;
	}
}

int ft_check_buf(void *buf, int size)
{
	char *recv_data;
	char c;
	static unsigned int iter = 0;
	int msg_index;
	int i;

	msg_index = ((iter++)*INTEG_SEED) % integ_alphabet_length;
	recv_data = (char *)buf;

	for (i = 0; i < size; i++) {
		c = integ_alphabet[msg_index++];
		if (msg_index >= integ_alphabet_length)
			msg_index = 0;
		if (c != recv_data[i])
			break;
	}
	if (i != size) {
		printf("Error at iteration=%d size=%d byte=%d\n",
			iter, size, i);
		return 1;
	}

	return 0;
}

uint64_t ft_init_cq_data(struct fi_info *info)
{
	if (info->domain_attr->cq_data_size >= sizeof(uint64_t)) {
		return 0x0123456789abcdefULL;
	} else {
		return 0x0123456789abcdef &
			((0x1ULL << (info->domain_attr->cq_data_size * 8)) - 1);
	}
}

int check_recv_msg(const char *message)
{
	size_t recv_len;
	size_t message_len = strlen(message) + 1;
	/* Account for null terminated byte. */
	recv_len = strlen(rx_buf) + 1;

	if (recv_len != message_len) {
		fprintf(stderr, "Received length does not match expected length.\n");
		return -1;
	}

	if (strncmp(rx_buf, message, message_len)) {
		fprintf(stderr, "Received message does not match expected message.\n");
		return -1;
	}
	fprintf(stdout, "Data check OK\n");
	return 0;
}

int ft_send_greeting(struct fid_ep *ep)
{
	size_t message_len = strlen(greeting) + 1;
	int ret;

	fprintf(stdout, "Sending message...\n");
	if (snprintf(tx_buf, tx_size, "%s", greeting) >= tx_size) {
		fprintf(stderr, "Transmit buffer too small.\n");
		return -FI_ETOOSMALL;
	}

	ret = ft_tx(ep, remote_fi_addrs[(rt_get_rank() + 1) % rt_get_size()], message_len, &tx_ctx);
	if (ret)
		return ret;

	fprintf(stdout, "Send completion received\n");
	return 0;
}

int ft_recv_greeting(struct fid_ep *ep)
{
	int ret;

	fprintf(stdout, "Waiting for message from client...\n");
	ret = ft_get_rx_comp(rx_seq);
	if (ret)
		return ret;

	ret = check_recv_msg(greeting);
	if (ret)
		return ret;

	fprintf(stdout, "Received data from client: %s\n", (char *) rx_buf);
	return 0;
}

int ft_send_recv_greeting(struct fid_ep *ep)
{
	return rt_get_rank() ? ft_send_greeting(ep) : ft_recv_greeting(ep);
}

int ft_sock_listen(char *node, char *service)
{
	struct addrinfo *ai, hints;
	int val, ret;

	memset(&hints, 0, sizeof hints);
	hints.ai_flags = AI_PASSIVE;

	ret = getaddrinfo(node, service, &hints, &ai);
	if (ret) {
		fprintf(stderr, "getaddrinfo() %s\n", gai_strerror(ret));
		return ret;
	}

	listen_sock = socket(ai->ai_family, SOCK_STREAM, 0);
	if (listen_sock < 0) {
		perror("socket");
		ret = listen_sock;
		goto out;
	}

	val = 1;
	ret = setsockopt(listen_sock, SOL_SOCKET, SO_REUSEADDR,
			 (void *) &val, sizeof val);
	if (ret) {
		perror("setsockopt SO_REUSEADDR");
		goto out;
	}

	ret = bind(listen_sock, ai->ai_addr, ai->ai_addrlen);
	if (ret) {
		perror("bind");
		goto out;
	}

	ret = listen(listen_sock, 0);
	if (ret)
		perror("listen");

out:
	if (ret && listen_sock >= 0)
		close(listen_sock);
	freeaddrinfo(ai);
	return ret;
}

int ft_sock_connect(char *node, char *service)
{
	struct addrinfo *ai;
	int ret;

	ret = getaddrinfo(node, service, NULL, &ai);
	if (ret) {
		perror("getaddrinfo");
		return ret;
	}

	sock = socket(ai->ai_family, SOCK_STREAM, 0);
	if (sock < 0) {
		perror("socket");
		ret = sock;
		goto free;
	}

	ret = 1;
	ret = setsockopt(sock, IPPROTO_TCP, TCP_NODELAY, (void *) &ret, sizeof(ret));
	if (ret)
		perror("setsockopt");

	ret = connect(sock, ai->ai_addr, ai->ai_addrlen);
	if (ret) {
		perror("connect");
		close(sock);
	}

free:
	freeaddrinfo(ai);
	return ret;
}

int ft_sock_accept()
{
	int ret, op;

	sock = accept(listen_sock, NULL, 0);
        if (sock < 0) {
		ret = sock;
		perror("accept");
		return ret;
	}

	op = 1;
	ret = setsockopt(sock, IPPROTO_TCP, TCP_NODELAY,
			  (void *) &op, sizeof(op));
	if (ret)
		perror("setsockopt");

	return 0;
}

int ft_sock_send(int fd, void *msg, size_t len)
{
	int ret;

	ret = send(fd, msg, len, 0);
	if (ret == len) {
		return 0;
	} else if (ret < 0) {
		perror("send");
		return -errno;
	} else {
		perror("send aborted");
		return -FI_ECONNABORTED;
	}
}

int ft_sock_recv(int fd, void *msg, size_t len)
{
	int ret;

	ret = recv(fd, msg, len, MSG_WAITALL);
	if (ret == len) {
		return 0;
	} else if (ret == 0) {
		return -FI_ENOTCONN;
	} else if (ret < 0) {
		FT_PRINTERR("ft_sock_recv", -errno);
		perror("recv");
		return -errno;
	} else {
		perror("recv aborted");
		return -FI_ECONNABORTED;
	}
}

int ft_sock_sync(int value)
{
	int result = -FI_EOTHER;

	if (listen_sock < 0) {
		ft_sock_send(sock, &value,  sizeof value);
		ft_sock_recv(sock, &result, sizeof result);
	} else {
		ft_sock_recv(sock, &result, sizeof result);
		ft_sock_send(sock, &value,  sizeof value);
	}

	return result;
}

void ft_sock_shutdown(int fd)
{
	shutdown(fd, SHUT_RDWR);
	close(fd);
}

static int ft_has_util_prefix(const char *str)
{
	return !strncasecmp(str, OFI_UTIL_PREFIX, strlen(OFI_UTIL_PREFIX));
}

const char *ft_util_name(const char *str, size_t *len)
{
	char *delim;

	delim = strchr(str, OFI_NAME_DELIM);
	if (delim) {
		if (ft_has_util_prefix(delim + 1)) {
			*len = strlen(delim + 1);
			return delim + 1;
		} else if (ft_has_util_prefix(str)) {
			*len = delim - str;
			return str;
		}
	} else if (ft_has_util_prefix(str)) {
		*len = strlen(str);
		return str;
	}
	*len = 0;
	return NULL;
}

const char *ft_core_name(const char *str, size_t *len)
{
	char *delim;

	delim = strchr(str, OFI_NAME_DELIM);
	if (delim) {
		if (!ft_has_util_prefix(delim + 1)) {
			*len = strlen(delim + 1);
			return delim + 1;
		} else if (!ft_has_util_prefix(str)) {
			*len = delim - str;
			return str;
		}
	} else if (!ft_has_util_prefix(str)) {
		*len = strlen(str);
		return str;
	}
	*len = 0;
	return NULL;
}

/* Split the given string "s" using the specified delimiter(s) in the string
 * "delim" and return an array of strings. The array is terminated with a NULL
 * pointer. Returned array should be freed with ft_free_string_array().
 *
 * Returns NULL on failure.
 */

char **ft_split_and_alloc(const char *s, const char *delim, size_t *count)
{
	int i, n;
	char *tmp;
	char *dup = NULL;
	char **arr = NULL;

	if (!s || !delim)
		return NULL;

	dup = strdup(s);
	if (!dup)
		return NULL;

	/* compute the array size */
	n = 1;
	for (tmp = dup; *tmp != '\0'; ++tmp) {
		for (i = 0; delim[i] != '\0'; ++i) {
			if (*tmp == delim[i]) {
				++n;
				break;
			}
		}
	}

	/* +1 to leave space for NULL terminating pointer */
	arr = calloc(n + 1, sizeof(*arr));
	if (!arr)
		goto cleanup;

	/* set array elts to point inside the dup'ed string */
	for (tmp = dup, i = 0; tmp != NULL; ++i) {
		arr[i] = strsep(&tmp, delim);
	}
	assert(i == n);

	if (count)
		*count = n;
	return arr;

cleanup:
	free(dup);
	free(arr);
	return NULL;
}

/* see ft_split_and_alloc() */
void ft_free_string_array(char **s)
{
	/* all strings are allocated from the same strdup'ed slab, so just free
	 * the first element */
	if (s != NULL)
		free(s[0]);

	/* and then the actual array of pointers */
	free(s);
}
