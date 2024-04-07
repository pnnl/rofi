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
#include <math.h>
#include <netdb.h>
#include <poll.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>

#include <rdma/fabric.h>
#include <rdma/fi_cm.h>
#include <rdma/fi_collective.h>
#include <rdma/fi_domain.h>
#include <rdma/fi_endpoint.h>
#include <rdma/fi_errno.h>
#include <rdma/fi_rma.h>

#include "rofi_debug.h"
#include "rofi_internal.h"
#include "transport.h"

int rofi_transport_fini(rofi_sub_transport_t *trans) {
    DEBUG_MSG("Fini");

    int ret = fi_close(&trans->ep->fid);
    if (ret) {
        ROFI_TRANSPORT_ERR_MSG("fi_close", ret);
        trans->ep = NULL;
    }
    DEBUG_MSG("ep closed");

    ret = fi_close(&trans->cq->fid);
    if (ret) {
        ROFI_TRANSPORT_ERR_MSG("fl_close", ret);
        trans->cq = NULL;
    }
    DEBUG_MSG("cq closed");

    ret = fi_close(&trans->put_cntr->fid);
    if (ret) {
        ROFI_TRANSPORT_ERR_MSG("fi_close", ret);
        trans->put_cntr = NULL;
    }
    DEBUG_MSG("put_cntr closed");

    ret = fi_close(&trans->get_cntr->fid);
    if (ret) {
        ROFI_TRANSPORT_ERR_MSG("fi_close", ret);
        trans->get_cntr = NULL;
    }
    DEBUG_MSG("get_cntr closed");

    ret = fi_close(&trans->av->fid);
    if (ret) {
        ROFI_TRANSPORT_ERR_MSG("fi_close", ret);
        trans->av = NULL;
    }
    DEBUG_MSG("av closed");

    ret = fi_close(&trans->eq->fid);
    if (ret) {
        ROFI_TRANSPORT_ERR_MSG("fi_close", ret);
        trans->eq = NULL;
    }
    DEBUG_MSG("eq closed");

    ret = fi_close(&trans->domain->fid);
    if (ret) {
        ROFI_TRANSPORT_ERR_MSG("fi_close", ret);
        trans->domain = NULL;
    }
    DEBUG_MSG("domain closed");

    ret = fi_close(&trans->fabric->fid);
    if (ret) {
        ROFI_TRANSPORT_ERR_MSG("fi_close", ret);
        trans->fabric = NULL;
    }
    DEBUG_MSG("fabric closed");
    free(trans->info);
    DEBUG_MSG("info freed");
    return 0;
}

void rofi_transport_select_provider(struct fi_info *prov, rofi_sub_transport_t *trans, rofi_names_t *names) {
    struct fi_info *prov_cur = prov;
    if (names == NULL) {
        trans->info = fi_dupinfo(prov_cur);
        WARN_MSG("No matches for the specified provider and/or domain, using Provider:  %s  Version: (%u.%u) Fabric: %s Domain: %s max_inject: %zu, max_msg: %zu, stx: %s, MR_RMA_EVENT: %s, msg: %s, rma: %s, read: %s, write: %s, remote_read: %s, remote_write: %s, rma_event: %s, atomic: %s, collective: %s",
                 prov_cur->fabric_attr->prov_name,
                 FI_MAJOR(prov_cur->fabric_attr->prov_version),
                 FI_MINOR(prov_cur->fabric_attr->prov_version),
                 prov_cur->fabric_attr->name,
                 prov_cur->domain_attr->name,
                 prov_cur->tx_attr->inject_size,
                 prov_cur->ep_attr->max_msg_size,
                 prov_cur->domain_attr->max_ep_stx_ctx == 0 ? "no" : "yes",
                 prov_cur->domain_attr->mr_mode & FI_MR_RMA_EVENT ? "yes" : " no",
                 prov_cur->caps & FI_MSG ? "yes" : "no",
                 prov_cur->caps & FI_RMA ? "yes" : "no",
                 prov_cur->caps & FI_READ ? "yes" : "no",
                 prov_cur->caps & FI_WRITE ? "yes" : "no",
                 prov_cur->caps & FI_REMOTE_READ ? "yes" : "no",
                 prov_cur->caps & FI_REMOTE_WRITE ? "yes" : "no",
                 prov_cur->caps & FI_RMA_EVENT ? "yes" : "no",
                 prov_cur->caps & FI_ATOMIC ? "yes" : "no",
                 prov_cur->caps & FI_COLLECTIVE ? "yes" : "no");
    }
    else {

        for (int i = 0; i < names->num; i++) {
            DEBUG_MSG("Checking Provider: %s", names->names[i]);
            while (prov_cur != NULL) {

                if (strncmp(prov_cur->fabric_attr->prov_name, names->names[i], strlen(names->names[i])) == 0 || strncmp(prov_cur->domain_attr->name, names->names[i], strlen(names->names[i])) == 0) {
                    DEBUG_MSG("Selected Provider: %s  Version: (%u.%u) Fabric: %s Domain: %s max_inject: %zu, max_msg: %zu, stx: %s, MR_RMA_EVENT: %s, msg: %s, rma: %s, read: %s, write: %s, remote_read: %s, remote_write: %s, rma_event: %s, atomic: %s, collective: %s",
                              prov_cur->fabric_attr->prov_name,
                              FI_MAJOR(prov_cur->fabric_attr->prov_version),
                              FI_MINOR(prov_cur->fabric_attr->prov_version),
                              prov_cur->fabric_attr->name,
                              prov_cur->domain_attr->name,
                              prov_cur->tx_attr->inject_size,
                              prov_cur->ep_attr->max_msg_size,
                              prov_cur->domain_attr->max_ep_stx_ctx == 0 ? "no" : "yes",
                              prov_cur->domain_attr->mr_mode & FI_MR_RMA_EVENT ? "yes" : " no",
                              prov_cur->caps & FI_MSG ? "yes" : "no",
                              prov_cur->caps & FI_RMA ? "yes" : "no",
                              prov_cur->caps & FI_READ ? "yes" : "no",
                              prov_cur->caps & FI_WRITE ? "yes" : "no",
                              prov_cur->caps & FI_REMOTE_READ ? "yes" : "no",
                              prov_cur->caps & FI_REMOTE_WRITE ? "yes" : "no",
                              prov_cur->caps & FI_RMA_EVENT ? "yes" : "no",
                              prov_cur->caps & FI_ATOMIC ? "yes" : "no",
                              prov_cur->caps & FI_COLLECTIVE ? "yes" : "no");
                    trans->info = fi_dupinfo(prov_cur);
                    break;
                }
                prov_cur = prov_cur->next;
            }
        }
    }
}

void rofi_transport_find_provider(struct fi_info *hints, rofi_sub_transport_t *trans, rofi_names_t *prov_names, rofi_names_t *domain_names) {
    DEBUG_MSG("fi_getinfo");
    struct fi_info *prov = fi_allocinfo();
    int ret = fi_getinfo(ROFI_FI_VERSION, NULL, NULL, 0, hints, &prov);
    if (ret) {
        ROFI_TRANSPORT_ERR_MSG("fi_getinfo", ret);
    }

    struct fi_info *prov_cur = prov; // trans->info;

#ifdef _DEBUG
    while (prov_cur != NULL) {
        DEBUG_MSG("Available Provider: %s  Version: (%u.%u) Fabric: %s Domain: %s max_inject: %zu, max_msg: %zu, stx: %s, MR_RMA_EVENT: %s, msg: %s, rma: %s, read: %s, write: %s, remote_read: %s, remote_write: %s, rma_event: %s, atomic: %s, collective: %s",
                  prov_cur->fabric_attr->prov_name,
                  FI_MAJOR(prov_cur->fabric_attr->prov_version),
                  FI_MINOR(prov_cur->fabric_attr->prov_version),
                  prov_cur->fabric_attr->name,
                  prov_cur->domain_attr->name,
                  prov_cur->tx_attr->inject_size,
                  prov_cur->ep_attr->max_msg_size,
                  prov_cur->domain_attr->max_ep_stx_ctx == 0 ? "no" : "yes",
                  prov_cur->domain_attr->mr_mode & FI_MR_RMA_EVENT ? "yes" : " no",
                  prov_cur->caps & FI_MSG ? "yes" : "no",
                  prov_cur->caps & FI_RMA ? "yes" : "no",
                  prov_cur->caps & FI_READ ? "yes" : "no",
                  prov_cur->caps & FI_WRITE ? "yes" : "no",
                  prov_cur->caps & FI_REMOTE_READ ? "yes" : "no",
                  prov_cur->caps & FI_REMOTE_WRITE ? "yes" : "no",
                  prov_cur->caps & FI_RMA_EVENT ? "yes" : "no",
                  prov_cur->caps & FI_ATOMIC ? "yes" : "no",
                  prov_cur->caps & FI_COLLECTIVE ? "yes" : "no");
        prov_cur = prov_cur->next;
    }
#endif
    if (prov_names != NULL) {
        for (int i = 0; i < prov_names->num; i++) {
            DEBUG_MSG("prov_names->names[%d] = %s", i, prov_names->names[i]);
        }
        rofi_transport_select_provider(prov, trans, prov_names);
    }
    if (trans->info == NULL) {
        if (domain_names != NULL) {
            for (int i = 0; i < domain_names->num; i++) {
                DEBUG_MSG("domain_names->names[%d] = %s", i, domain_names->names[i]);
            }
            rofi_transport_select_provider(prov, trans, domain_names);
        }
    }
    if (trans->info == NULL && prov_names == NULL && domain_names == NULL) {
        rofi_transport_select_provider(prov, trans, NULL);
    }
    fi_freeinfo(prov);
}

int rofi_transport_init(struct fi_info *hints, rofi_sub_transport_t *trans, rofi_names_t *prov_names, rofi_names_t *domain_names) {
    rofi_transport_find_provider(hints, trans, prov_names, domain_names);
    if (trans->info == NULL) {
        hints->caps = hints->caps ^ FI_COLLECTIVE; // try without collective
        DEBUG_MSG("No providers found. Trying without collective...");
        rofi_transport_find_provider(hints, trans, prov_names, domain_names);
        if (trans->info == NULL) {
            rofi_transport_find_provider(hints, trans, NULL, NULL); // try to find a single provider
            if (trans->info == NULL) {
                ERR_MSG("Error initializing ROFI. No matching provider found. Aborting.");
                return -1;
            }
        }
    }

    int ret = rofi_transport_init_fabric_resources(trans);
    if (ret) {
        // already would have printed the error.
        return ret;
    }
    // if (strncmp(trans->info->fabric_attr->prov_name, "verbs", 5)) {
    //     ERR_MSG(" Only 'verbs' fabric is supported. Aborting.");
    //     return -1;
    // }

    trans->desc.max_message_size = trans->info->ep_attr->max_msg_size;
    trans->desc.inject_size = trans->info->tx_attr->inject_size;

    struct fi_collective_attr attr = {0};
    attr.op = FI_ATOMIC_READ;
    attr.datatype = FI_UINT64;
    attr.mode = 0;
    DEBUG_MSG("fi_query_collective: FI_ALLGATHER");
    ret = fi_query_collective(trans->domain, FI_ALLGATHER, &attr, 0);
    if (ret) {
        ROFI_TRANSPORT_ERR_MSG("fi_query_collective", ret);
        trans->fi_collective = 0;
        // return (ret);
    }
    else {
        trans->fi_collective = FI_COLLECTIVE;
        DEBUG_MSG("fi_query_collective: FI_ALLGATHER supported");
    }

    trans->fi_collective = 0;

    ret = rofi_transport_init_endpoint_resources(trans);
    if (ret) {
        // already would have printed the error.
        return ret;
    }

    char epname[512];
    size_t len = 64;
    ret = fi_getname(&trans->ep->fid, &epname, &len);
    if (ret) {
        ROFI_TRANSPORT_ERR_MSG("fi_getname", ret);
        return ret;
    }
    char buf[256];
    size_t buflen = 256;
    DEBUG_MSG("epname: %s", fi_av_straddr(trans->av, epname, buf, &buflen));
    rt_put("epname_len", &len, sizeof(size_t));
    rt_put("epname", epname, len);
    trans->desc.addrlen = len;
    rt_exchange();

    ret = rofi_transport_init_av(trans);
    if (ret) {
        // already would have printed the error.
        return ret;
    }
    return 0;
}

int rofi_transport_init_fabric_resources(rofi_sub_transport_t *trans) {
    DEBUG_MSG("FI_FABRIC");
    int ret = fi_fabric(trans->info->fabric_attr, &trans->fabric, NULL);
    if (ret) {
        ROFI_TRANSPORT_ERR_MSG("fi_fabric", ret);
        return ret;
    }

    struct fi_eq_attr eq_attr = {0};
    eq_attr.wait_obj = FI_WAIT_UNSPEC;
    DEBUG_MSG("FI_EQ_OPEN");
    ret = fi_eq_open(trans->fabric, &eq_attr, &trans->eq, NULL);
    if (ret) {
        ROFI_TRANSPORT_ERR_MSG("fi_eq_open", ret);
        return ret;
    }

    DEBUG_MSG("FI_DOMAIN");
    ret = fi_domain(trans->fabric, trans->info, &trans->domain, NULL);
    if (ret) {
        ROFI_TRANSPORT_ERR_MSG("fi_domain", ret);
        return ret;
    }

    return 0;
}
int rofi_transport_init_endpoint_resources(rofi_sub_transport_t *trans) {
    struct fi_cntr_attr put_cntr_attr = {0};
    struct fi_cntr_attr get_cntr_attr = {0};
    struct fi_cntr_attr send_cntr_attr = {0};
    struct fi_cntr_attr recv_cntr_attr = {0};
    put_cntr_attr.events = FI_CNTR_EVENTS_COMP;
    get_cntr_attr.events = FI_CNTR_EVENTS_COMP;
    send_cntr_attr.events = FI_CNTR_EVENTS_COMP;
    recv_cntr_attr.events = FI_CNTR_EVENTS_COMP;
    put_cntr_attr.wait_obj = FI_WAIT_UNSPEC;
    get_cntr_attr.wait_obj = FI_WAIT_UNSPEC;
    send_cntr_attr.wait_obj = FI_WAIT_UNSPEC;
    recv_cntr_attr.wait_obj = FI_WAIT_UNSPEC;

    DEBUG_MSG("put FI_CNTR_OPEN");
    int ret = fi_cntr_open(trans->domain, &put_cntr_attr, &trans->put_cntr, NULL);
    if (ret) {
        ROFI_TRANSPORT_ERR_MSG("fi_cntr_open", ret);
        return ret;
    }

    DEBUG_MSG("get FI_CNTR_OPEN");
    ret = fi_cntr_open(trans->domain, &get_cntr_attr, &trans->get_cntr, NULL);
    if (ret) {
        ROFI_TRANSPORT_ERR_MSG("fi_cntr_open", ret);
        return ret;
    }

    DEBUG_MSG("send FI_CNTR_OPEN");
    ret = fi_cntr_open(trans->domain, &send_cntr_attr, &trans->send_cntr, NULL);
    if (ret) {
        ROFI_TRANSPORT_ERR_MSG("fi_cntr_open", ret);
        return ret;
    }

    DEBUG_MSG("recv FI_CNTR_OPEN");
    ret = fi_cntr_open(trans->domain, &recv_cntr_attr, &trans->recv_cntr, NULL);
    if (ret) {
        ROFI_TRANSPORT_ERR_MSG("fi_cntr_open", ret);
        return ret;
    }

    struct fi_cq_attr cq_attr = {0};
    cq_attr.format = FI_CQ_FORMAT_CONTEXT;
    cq_attr.wait_obj = FI_WAIT_UNSPEC;

    DEBUG_MSG("FI_CQ_OPEN");
    ret = fi_cq_open(trans->domain, &cq_attr, &trans->cq, NULL);
    if (ret) {
        ROFI_TRANSPORT_ERR_MSG("fi_cq_open", ret);
        return ret;
    }

    struct fi_av_attr av_attr = {0};
    if (trans->info->domain_attr->av_type != FI_AV_UNSPEC) {
        av_attr.type = trans->info->domain_attr->av_type;
    }

    DEBUG_MSG("FI_AV_OPEN");
    ret = fi_av_open(trans->domain, &av_attr, &trans->av, NULL);
    if (ret) {
        ROFI_TRANSPORT_ERR_MSG("fi_av_open", ret);
        return ret;
    }

    trans->info->ep_attr->tx_ctx_cnt = 0;
    trans->info->caps = FI_RMA | FI_WRITE | FI_READ | FI_REMOTE_WRITE | FI_REMOTE_READ | trans->fi_collective;
    trans->info->tx_attr->op_flags = FI_DELIVERY_COMPLETE; // FI_TRANSMIT_COMPLETE fails, FI_DELIVERY_COMPLETE works but I dont see a difference?
    trans->info->mode = 0;
    trans->info->tx_attr->mode = 0;
    trans->info->rx_attr->mode = 0;
    trans->info->rx_attr->size = 1024;
    trans->info->tx_attr->size = 1024;
    trans->info->tx_attr->caps = trans->info->caps;
    trans->info->rx_attr->caps = FI_RECV | trans->fi_collective; // to drive progress

    DEBUG_MSG("FI_ENDPOINT");
    ret = fi_endpoint(trans->domain, trans->info, &trans->ep, NULL);
    if (ret) {
        ROFI_TRANSPORT_ERR_MSG("fi_endpoint", ret);
        return ret;
    }

    // bind event queue
    DEBUG_MSG("FI_EP_BIND eq");
    ret = fi_ep_bind(trans->ep, &trans->eq->fid, 0);
    if (ret) {
        ROFI_TRANSPORT_ERR_MSG("fi_ep_bind eq", ret);
        return ret;
    }

    // bind address vector
    DEBUG_MSG("FI_EP_BIND av");
    ret = fi_ep_bind(trans->ep, &trans->av->fid, 0);
    if (ret) {
        ROFI_TRANSPORT_ERR_MSG("fi_ep_bind av", ret);
        return ret;
    }

    // bind put cntr
    DEBUG_MSG("FI_EP_BIND put_cntr");
    ret = fi_ep_bind(trans->ep, &trans->put_cntr->fid, FI_WRITE | FI_REMOTE_WRITE);
    if (ret) {
        ROFI_TRANSPORT_ERR_MSG("fi_ep_bind put_cntr", ret);
        return ret;
    }

    // bind get cntr
    DEBUG_MSG("FI_EP_BIND get_cntr");
    ret = fi_ep_bind(trans->ep, &trans->get_cntr->fid, FI_READ | FI_REMOTE_READ);
    if (ret) {
        ROFI_TRANSPORT_ERR_MSG("fi_ep_bind get_cntr", ret);
        return ret;
    }

    // bind send cntr
    DEBUG_MSG("FI_EP_BIND send_cntr");
    ret = fi_ep_bind(trans->ep, &trans->send_cntr->fid, FI_SEND);
    if (ret) {
        ROFI_TRANSPORT_ERR_MSG("fi_ep_bind send_cntr", ret);
        return ret;
    }

    // bind recv cntr
    DEBUG_MSG("FI_EP_BIND recv_cntr");
    ret = fi_ep_bind(trans->ep, &trans->recv_cntr->fid, FI_RECV);
    if (ret) {
        ROFI_TRANSPORT_ERR_MSG("fi_ep_bind get_cntr", ret);
        return ret;
    }

    // bind cq -- use same completion queue for send and recv
    DEBUG_MSG("FI_EP_BIND cq");
    ret = fi_ep_bind(trans->ep, &trans->cq->fid, FI_SELECTIVE_COMPLETION | FI_TRANSMIT | FI_RECV);
    if (ret) {
        ROFI_TRANSPORT_ERR_MSG("fi_ep_bind cq", ret);
        return ret;
    }

    DEBUG_MSG("FI_ENABLE");
    ret = fi_enable(trans->ep);
    if (ret) {
        ROFI_TRANSPORT_ERR_MSG("fi_enable", ret);
        return ret;
    }
    return ret;
}

int rofi_transport_init_av(rofi_sub_transport_t *trans) {
    char *all_addrs = (char *)malloc(trans->desc.pes * trans->desc.addrlen);
    assert(all_addrs);

    int num_nodes = 0;
    for (int i = 0; i < trans->desc.pes; i++) {
        if (!trans->local || rt_get_node_rank(i) != -1) {
            char *addr_ptr = all_addrs + i * trans->desc.addrlen;
            char buf[256];
            size_t buflen = 256;
            int ret = rt_get(i, "epname", addr_ptr, trans->desc.addrlen);
            DEBUG_MSG("Got EP address name from %i (%s).", i, fi_av_straddr(trans->av, addr_ptr, buf, &buflen));
            if (ret) {
                ERR_MSG("Error getting EP address name from %i (%d).", i, ret);
                return ret;
            }
            num_nodes += 1;
        }
    }

    DEBUG_MSG("FI_AV_INSERT");
    int ret = fi_av_insert(trans->av, all_addrs, num_nodes, trans->remote_addrs, 0, NULL);
    free(all_addrs);
    if (ret < 0) {
        ROFI_TRANSPORT_ERR_MSG("ft_av_insert", ret);
        return ret;
    }
    else if (ret != num_nodes) {
        ERR_MSG("fi_av_insert: number of addresses inserted = %d;"
                " number of addresses given = %d\n",
                ret, num_nodes);
        return ret;
    }
    return 0;
}

// only need this if we use MANUAL_PROGRESS
// because we are using FI_SELECTIVE_COMPLETION
// successfull completions should increment the respective cntrs
// this shouldn't return any completions, as thc cq should
// only handle error events now
int rofi_transport_progress(rofi_sub_transport_t *trans) {
    struct fi_cq_entry buf = {0};
    int ret = fi_cq_read(trans->cq, &buf, 1);
    if (ret == 1) {
        printf("unexpected cq event\n"); // warnd
    }
    else if (ret < 0 && ret != -FI_EAGAIN) {
        ROFI_TRANSPORT_ERR_MSG("fi_cq_read", ret);
        struct fi_cq_err_entry ebuf = {0};
        int ret = fi_cq_readerr(trans->cq, (void *)&ebuf, 0);
        if (ret > 0) {
            const char *errmsg = fi_cq_strerror(trans->cq, ebuf.prov_errno, ebuf.err_data, NULL, 0);
            ERR_MSG("Error: %s\n", errmsg);
            abort();
            return ret;
        }
        else if (ret && ret != -FI_EAGAIN) {
            ROFI_TRANSPORT_ERR_MSG("fi_cq_readerr", ret);
            return ret;
        }
        return (ret);
    }
    return 0;
}

// checks the rma return status, and aborts if not -FI_EAGAIN
// note this does not try to make progress
int rofi_transport_locked_ctx_check_err(rofi_sub_transport_t *trans, int err) {
    if (err == -FI_EAGAIN) {
        struct fi_cq_err_entry ebuf = {0};
        int ret = fi_cq_readerr(trans->cq, (void *)&ebuf, 0);
        if (ret > 0) {
            const char *errmsg = fi_cq_strerror(trans->cq, ebuf.prov_errno, ebuf.err_data, NULL, 0);
            ERR_MSG("Error: %s\n", errmsg);
            return ret;
        }
        else if (ret && ret != -FI_EAGAIN) {
            ROFI_TRANSPORT_ERR_MSG("fi_cq_readerr", ret);
            return ret;
        }
    }
    else if (err) {
        ROFI_TRANSPORT_ERR_MSG("", err);
        return err;
    }
    return 0;
}

// checks the rma return status, and aborts if not -FI_EAGAIN
// note this does not try to make progress
int rofi_transport_ctx_check_err(rofi_sub_transport_t *trans, int err) {
    if (err == -FI_EAGAIN) {
        struct fi_cq_err_entry ebuf = {0};
        pthread_mutex_lock(&trans->lock);
        int ret = fi_cq_readerr(trans->cq, (void *)&ebuf, 0);
        pthread_mutex_unlock(&trans->lock);
        if (ret > 0) {
            const char *errmsg = fi_cq_strerror(trans->cq, ebuf.prov_errno, ebuf.err_data, NULL, 0);
            ERR_MSG("Error: %s\n", errmsg);
            return ret;
        }
        else if (ret && ret != -FI_EAGAIN) {
            ROFI_TRANSPORT_ERR_MSG("fi_cq_readerr", ret);
            return ret;
        }
    }
    else if (err) {
        ROFI_TRANSPORT_ERR_MSG("", err);
        return err;
    }
    return 0;
}

// checks the rma return status, and aborts if not -FI_EAGAIN
// otherwise tries to make progress
int rofi_transport_check_rma_err(rofi_sub_transport_t *trans, int err) {
    if (err == -FI_EAGAIN) {
        struct fi_cq_err_entry ebuf = {0};
        int ret = fi_cq_readerr(trans->cq, (void *)&ebuf, 0);
        if (ret > 0) {
            const char *errmsg = fi_cq_strerror(trans->cq, ebuf.prov_errno, ebuf.err_data, NULL, 0);
            printf("Error: %s\n", errmsg);
            return ret;
        }
        else if (ret && ret != -FI_EAGAIN) {
            ROFI_TRANSPORT_ERR_MSG("fi_cq_readerr", ret);
            return ret;
        }
        ret = rofi_transport_progress(trans);
        if (ret) {
            return ret;
        }
    }
    else if (err) {
        ROFI_TRANSPORT_ERR_MSG("", err);
        return err;
    }
    return 0;
}

int rofi_transport_locked_wait_on_cntr(rofi_sub_transport_t *trans, uint64_t *pending_cntr, struct fid_cntr *cntr) {
    uint64_t cnt = *pending_cntr;
    uint64_t prev_cnt = cnt;
    uint64_t cur_cnt = fi_cntr_read(cntr);
    uint64_t err_cnt = fi_cntr_readerr(cntr);
    DEBUG_MSG("Waiting for  %lu  cnts... cur_cnt: %lu err_cnt: %lu", cnt, cur_cnt, err_cnt);
    do {
        prev_cnt = cnt;
        int ret = fi_cntr_wait(cntr, prev_cnt, -1);
        cnt = *pending_cntr; // this could be updated by another thread
        ret = rofi_transport_locked_ctx_check_err(trans, ret);
        ret = rofi_transport_progress(trans);
        if (ret) {
            return ret;
        }
    } while (prev_cnt < cnt);
    // pthread_mutex_lock(&trans->lock);
    // uint64_t cnt = fi_cntr_read(cntr);
    // uint64_t err_cnt = fi_cntr_readerr(cntr);
    // pthread_mutex_unlock(&trans->lock);
    // DEBUG_MSG("Done Waiting for  %lu  prev_cnt: %lu gets to complete... cnt: %lu err_cnt: %lu", *pending_cntr, prev_cnt, cnt, err_cnt);
    assert(prev_cnt == cnt);
    return 0;
}

int rofi_transport_wait_on_cntr(rofi_sub_transport_t *trans, uint64_t *pending_cntr, struct fid_cntr *cntr) {
    uint64_t cnt = *pending_cntr;
    uint64_t prev_cnt = cnt;
    pthread_mutex_lock(&trans->lock);
    uint64_t cur_cnt = fi_cntr_read(cntr);
    uint64_t err_cnt = fi_cntr_readerr(cntr);
    pthread_mutex_unlock(&trans->lock);
    DEBUG_MSG("Waiting for  %lu  cnts... cur_cnt: %lu err_cnt: %lu", cnt, cur_cnt, err_cnt);
    do {
        prev_cnt = cnt;
        pthread_mutex_lock(&trans->lock);
        int ret = fi_cntr_wait(cntr, prev_cnt, -1);
        pthread_mutex_unlock(&trans->lock);
        cnt = *pending_cntr; // this could be updated by another thread
        ret = rofi_transport_ctx_check_err(trans, ret);
        if (ret) {
            return ret;
        }
    } while (prev_cnt < cnt);
    // pthread_mutex_lock(&trans->lock);
    // uint64_t cnt = fi_cntr_read(cntr);
    // uint64_t err_cnt = fi_cntr_readerr(cntr);
    // pthread_mutex_unlock(&trans->lock);
    // DEBUG_MSG("Done Waiting for  %lu  prev_cnt: %lu gets to complete... cnt: %lu err_cnt: %lu", *pending_cntr, prev_cnt, cnt, err_cnt);
    assert(prev_cnt == cnt);
    return 0;
}

int rofi_transport_wait_on_event(rofi_sub_transport_t *trans, uint32_t event, void *context) {
    uint32_t ev;
    struct fi_eq_entry entry;

    while (true) {
        int ret = fi_eq_read(trans->eq, &ev, &entry, sizeof(entry), 0);
        if (ret >= 0) { // we got an event
            if (ev == event) {
                if (!context || (context == entry.context)) {
                    return 0;
                }
                else if (context) {
                    return -FI_EOTHER;
                }
            }
        }
        else if (ret != -FI_EAGAIN) {
            ROFI_TRANSPORT_ERR_MSG("fi_eq_read", ret);
            return ret;
        }
        ret = rofi_transport_progress(trans);
        if (ret) {
            return ret;
        }
    }
}

int rofi_transport_wait_on_context_comp(rofi_sub_transport_t *trans, void *context) {
    struct fi_cq_entry buf = {0};
    struct fi_cq_err_entry err_entry = {0};

    DEBUG_MSG("Waiting on context comp %p", context);

    while (true) {
        int ret = fi_cq_read(trans->cq, &buf, 1);
        if (ret < 0 && ret != -FI_EAGAIN) {
            ROFI_TRANSPORT_ERR_MSG("fi_cq_read", ret);
            return ret;
        }
        if (buf.op_context && buf.op_context == context) {
            return 0;
        }
        else if (buf.op_context) {
            DEBUG_MSG("Unexpected context comp %p != %p", buf.op_context, context);
        }
    }
}

int rofi_transport_put_inject(rofi_sub_transport_t *trans, struct fi_rma_iov *rma_iov, fi_addr_t pe, const void *src_addr, size_t len) {
    pthread_mutex_lock(&trans->lock);
    trans->pending_put_cntr += 1;
    DEBUG_MSG("fi_inject_write %p %p %d %d %p 0x%lx", trans->ep, src_addr, len, pe, rma_iov->addr, rma_iov->key);
    int ret = fi_inject_write(trans->ep, src_addr, len, pe, rma_iov->addr, rma_iov->key);
    while (ret) { // retry while FI_EAGAIN
        ret = rofi_transport_check_rma_err(trans, ret);
        if (ret) {
            return ret;
        }
        ret = fi_inject_write(trans->ep, src_addr, len, pe, rma_iov->addr, rma_iov->key);
    }
    pthread_mutex_unlock(&trans->lock);
    DEBUG_MSG("fi_inject_write done %p %p %d %d %p 0x%lx", trans->ep, src_addr, len, pe, rma_iov->addr, rma_iov->key);
    return 0;
}

int rofi_transport_put_large(rofi_sub_transport_t *trans, struct fi_rma_iov *rma_iov, fi_addr_t pe, const void *src_addr, size_t len, void *desc, void *context) {

    uint8_t *src_cur_addr = (uint8_t *)src_addr;
    uint8_t *src_end_addr = src_cur_addr + len;
    uint64_t dst_cur_addr = (uint64_t)rma_iov->addr;
    pthread_mutex_lock(&trans->lock);
    DEBUG_MSG("fi_write %p %p %d %d %p 0x%lx", trans->ep, src_cur_addr, len, pe, rma_iov->addr, rma_iov->key);
    while (src_cur_addr < src_end_addr) {
        uint64_t cur_len = MIN(src_end_addr - src_cur_addr, trans->desc.max_message_size);
        trans->pending_put_cntr += 1;

        int ret = fi_write(trans->ep, src_cur_addr, cur_len, desc, pe, dst_cur_addr, rma_iov->key, context);

        while (ret) { // retry while FI_EAGAIN
            ret = rofi_transport_check_rma_err(trans, ret);
            if (ret) {
                return ret;
            }
            ret = fi_write(trans->ep, src_cur_addr, cur_len, desc, pe, dst_cur_addr, rma_iov->key, context);
        }
        src_cur_addr += cur_len;
        dst_cur_addr += cur_len;
    }
    pthread_mutex_unlock(&trans->lock);
    DEBUG_MSG("fi_inject_write %p %p %d %d %p 0x%lx", trans->ep, src_addr, len, pe, rma_iov->addr, rma_iov->key);
    return 0;
}

// for PE need to check if using FI_AV_MAP, and then index into that
int rofi_transport_put(rofi_sub_transport_t *trans, struct fi_rma_iov *rma_iov, uint64_t pe, const void *src_addr, size_t len, void *desc, void *context) {
    if (len < trans->desc.inject_size) {
        return rofi_transport_put_inject(trans, rma_iov, trans->remote_addrs[pe], src_addr, len);
    }
    else {
        return rofi_transport_put_large(trans, rma_iov, trans->remote_addrs[pe], src_addr, len, desc, context);
    }
}

int rofi_transport_put_wait_all(rofi_sub_transport_t *trans) {
    return rofi_transport_wait_on_cntr(trans, &trans->pending_put_cntr, trans->put_cntr);
}

int rofi_transport_get_small(rofi_sub_transport_t *trans, struct fi_rma_iov *rma_iov, uint64_t pe, void *dst_addr, size_t len, void *desc, void *context) {

    pthread_mutex_lock(&trans->lock);
    trans->pending_get_cntr += 1;
    int ret = fi_read(trans->ep, dst_addr, len, desc, pe, rma_iov->addr, rma_iov->key, context);
    while (ret) { // retry while FI_EAGAIN
        ret = rofi_transport_check_rma_err(trans, ret);
        if (ret) {
            return ret;
        }
        ret = fi_read(trans->ep, dst_addr, len, desc, pe, rma_iov->addr, rma_iov->key, context);
    }
    pthread_mutex_unlock(&trans->lock);
    return 0;
}

int rofi_transport_get_large(rofi_sub_transport_t *trans, struct fi_rma_iov *rma_iov, uint64_t pe, void *dst_addr, size_t len, void *desc, void *context) {
    uint64_t src_cur_addr = (uint64_t)rma_iov->addr;

    uint8_t *dst_cur_addr = (uint8_t *)dst_addr;
    uint8_t *dst_end_addr = dst_cur_addr + len;
    pthread_mutex_lock(&trans->lock);
    while (dst_cur_addr < dst_end_addr) {
        uint64_t cur_len = MIN(dst_end_addr - dst_cur_addr, trans->desc.max_message_size);
        trans->pending_get_cntr += 1;

        int ret = fi_read(trans->ep, dst_cur_addr, cur_len, desc, pe, src_cur_addr, rma_iov->key, context);

        while (ret) { // retry while FI_EAGAIN
            ret = rofi_transport_check_rma_err(trans, ret);
            if (ret) {
                return ret;
            }
            ret = fi_read(trans->ep, dst_cur_addr, cur_len, desc, pe, src_cur_addr, rma_iov->key, context);
        }
        src_cur_addr += cur_len;
        dst_cur_addr += cur_len;
    }
    pthread_mutex_unlock(&trans->lock);
    return 0;
}

int rofi_transport_get(rofi_sub_transport_t *trans, struct fi_rma_iov *rma_iov, uint64_t pe, void *dst_addr, size_t len, void *desc, void *context) {
    if (len < trans->desc.max_message_size) {
        return rofi_transport_get_small(trans, rma_iov, trans->remote_addrs[pe], dst_addr, len, desc, context);
    }
    else {
        return rofi_transport_get_large(trans, rma_iov, trans->remote_addrs[pe], dst_addr, len, desc, context);
    }
}

int rofi_transport_get_wait_all(rofi_sub_transport_t *trans) {
    return rofi_transport_wait_on_cntr(trans, &trans->pending_get_cntr, trans->get_cntr);
}

int rofi_transport_send(rofi_sub_transport_t *trans, void *buf, size_t len, uint64_t pe) {
    pthread_mutex_lock(&trans->lock);
    uint64_t finish_flag = 0;
    trans->pending_send_cntr += 1;
    int ret = fi_send(trans->ep, buf, len, NULL, trans->remote_addrs[pe], &finish_flag);
    while (ret) { // retry while FI_EAGAIN
        ret = rofi_transport_check_rma_err(trans, ret);
        if (ret) {
            pthread_mutex_unlock(&trans->lock);
            return ret;
        }
        ret = fi_send(trans->ep, buf, len, NULL, trans->remote_addrs[pe], &finish_flag);
    }
    rofi_transport_locked_wait_on_cntr(trans, &trans->pending_send_cntr, trans->send_cntr);
    pthread_mutex_unlock(&trans->lock);

    return 0;
}

// we actually need to make this async, so store the finish flag in a hashmap, that we can use to check on subsequent calls
int rofi_transport_recv(rofi_sub_transport_t *trans, void *buf, size_t len) {
    pthread_mutex_lock(&trans->lock);
    uint64_t finish_flag = 0;
    trans->pending_recv_cntr += 1;
    int ret = fi_recv(trans->ep, buf, len, NULL, 0, &finish_flag);
    while (ret) { // retry while FI_EAGAIN
        ret = rofi_transport_check_rma_err(trans, ret);
        if (ret) {
            pthread_mutex_unlock(&trans->lock);
            return ret;
        }
        ret = fi_recv(trans->ep, buf, len, NULL, 0, &finish_flag);
    }
    rofi_transport_locked_wait_on_cntr(trans, &trans->pending_recv_cntr, trans->recv_cntr);
    pthread_mutex_unlock(&trans->lock);

    return 0;
}

int rofi_transport_exchange_mr_info(rofi_transport_t *rofi, rofi_mr_desc *mr) {
    if (rofi->dist->desc.pes == 1) {
        return 0;
    }

    struct fi_rma_iov rma_iov;
    rma_iov.addr = (uint64_t)mr->start;
    rma_iov.key = fi_mr_key(mr->fid);
    DEBUG_MSG("Exchanging MR Info (key: 0x%lx, addr: 0x%lx)....", rma_iov.key, rma_iov.addr);

    int ret = rt_exchange_data("mr_info", &rma_iov, sizeof(struct fi_rma_iov), mr->iov, rofi->dist->desc.pe_id, rofi->dist->desc.pes);
    if (ret) {
        ERR_MSG("Error exchanging info for memory region alloc buffer. Aborting!");
        return ret;
    }

#ifdef _DEBUG
    for (int i = 0; i < rofi->dist->desc.pes; i++) {
        DEBUG_MSG("\t Node: %d Key: 0x%lx Addr: 0x%lx", i, mr->iov[i].key, mr->iov[i].addr);
    }
#endif
    return 0;
}

// for use when FI_COLLECTIVE not available
int rofi_transport_sub_exchange_mr_info_manual(rofi_transport_t *rofi, rofi_mr_desc *mr, uint64_t *pes, uint64_t num_pes) {
    int me = rofi->dist->desc.pe_id;
    if (pes != NULL) { // doing sub barrier, figure out team pe id
        for (int i = 0; i < num_pes; i++) {
            if (pes[i] == me) {
                me = i;
                break;
            }
        }
    }
    struct fi_rma_iov *sub_alloc_buf = rofi->sub_alloc_buf;
    sub_alloc_buf[me].addr = (uint64_t)mr->start;
    sub_alloc_buf[me].key = fi_mr_key(mr->fid);
    DEBUG_MSG("Placing mr info (key: 0x%lx, addr: 0x%lx)....", sub_alloc_buf[me].key, sub_alloc_buf[me].addr);
    uint64_t sub_alloc_barrier_id = 0;
    rofi_transport_inner_barrier(rofi, &sub_alloc_barrier_id, rofi->sub_alloc_barrier_buf, pes, me, num_pes);

    for (int pe = me + 1; pe < num_pes; pe++) {
        uint64_t actual_pe = pes[pe];
        void *src = (void *)&sub_alloc_buf[actual_pe]; // this will be translated to the remote PE
        void *dst = src;                               // this will be our local data

        rofi_get_internal(dst, src, sizeof(struct fi_rma_iov), actual_pe, 0);
    }
    for (int pe = 0; pe < me; pe++) {
        uint64_t actual_pe = pes[pe];
        void *src = (void *)&sub_alloc_buf[actual_pe]; // this will be translated to the remote PE
        void *dst = src;                               // this will be our local data

        rofi_get_internal(dst, src, sizeof(struct fi_rma_iov), actual_pe, 0);
    }
    if (rofi->shm) {
        if (rofi_transport_get_wait_all(rofi->shm)) {
            ERR_MSG("\t Error waiting for shm get in barrier");
        }
    }
    if (rofi_transport_get_wait_all(rofi->dist)) {
        ERR_MSG("\t Error waiting for get in barrier");
    }

    for (int pe = 0; pe < num_pes; pe++) {
        uint64_t actual_pe = pes[pe];
        mr->iov[actual_pe] = sub_alloc_buf[actual_pe];
        DEBUG_MSG("i: %d(pe: %d), addr: 0x%lx, key: 0x%lx  ", pe, actual_pe, sub_alloc_buf[actual_pe].addr, sub_alloc_buf[actual_pe].key);
    }
    rofi_transport_inner_barrier(rofi, &sub_alloc_barrier_id, rofi->sub_alloc_barrier_buf, pes, me, num_pes);

    return 0;
}

int rofi_transport_sub_exchange_mr_info(rofi_transport_t *rofi, rofi_mr_desc *mr, uint64_t *pes, uint64_t num_pes) {
    if (rofi->dist->desc.pes == 1) {
        return 0;
    }
    if (!rofi->dist->fi_collective) {
        return rofi_transport_sub_exchange_mr_info_manual(rofi, mr, pes, num_pes);
    }

    int me = rofi->dist->desc.pe_id;
    if (pes != NULL) { // doing sub barrier, figure out team pe id
        for (int i = 0; i < num_pes; i++) {
            if (pes[i] == me) {
                me = i;
                break;
            }
        }
    }

    DEBUG_MSG("Broadcasting MR Info (key: 0x%lx, 0x%lx, addr: 0x%lx) to %d PEs....", mr->mr_key, fi_mr_key(mr->fid), mr->start, num_pes);
    struct fi_av_set_attr av_set_attr = {0};
    av_set_attr.count = num_pes;
    av_set_attr.start_addr = rofi->dist->remote_addrs[pes[0]];
    av_set_attr.end_addr = rofi->dist->remote_addrs[pes[0]];
    av_set_attr.stride = 1;
    // av_set_attr.comm_key_size = 0; // need to look into comm keys more
    // av_set_attr.comm_key = 0;
    av_set_attr.flags = 0;

    struct fid_av_set *av_set;
    pthread_mutex_lock(&rofi->dist->lock);
    int ret = fi_av_set(rofi->dist->av, &av_set_attr, &av_set, NULL);
    if (ret) {
        ROFI_TRANSPORT_ERR_MSG("fi_av_st", ret);
        pthread_mutex_unlock(&rofi->dist->lock);
        return ret;
    }
    DEBUG_MSG("CREATED AV_SET: 0x%p", av_set);

    for (int i = 1; i < num_pes; i++) {
        ret = fi_av_set_insert(av_set, rofi->dist->remote_addrs[pes[i]]);
        if (ret) {
            ROFI_TRANSPORT_ERR_MSG("fi_av_set_insert", ret);
            pthread_mutex_unlock(&rofi->dist->lock);
            return ret;
        }
        DEBUG_MSG("Inserted PE %d into AV_SET: 0x%p", pes[i], av_set);
    }
    fi_addr_t coll_addr = 0;
    ret = fi_av_set_addr(av_set, &coll_addr);
    if (ret) {
        ROFI_TRANSPORT_ERR_MSG("fi_av_set_addr", ret);
        pthread_mutex_unlock(&rofi->dist->lock);
        return ret;
    }

    DEBUG_MSG("COLL_ADDR: 0x%p", coll_addr);

    uint64_t done_flag;
    struct fid_mc *mc;
    ret = fi_join_collective(rofi->dist->ep, coll_addr, av_set, 0, &mc, &done_flag);
    if (ret) {
        ROFI_TRANSPORT_ERR_MSG("fi_join_collective", ret);
        pthread_mutex_unlock(&rofi->dist->lock);
        return ret;
    }
    DEBUG_MSG("Initiated collective join...");
    ret = rofi_transport_wait_on_event(rofi->dist, FI_JOIN_COMPLETE, &done_flag);
    if (ret) {
        ROFI_TRANSPORT_ERR_MSG("rofi_transport_wait_on_event", ret);
        pthread_mutex_unlock(&rofi->dist->lock);
        return ret;
    }
    DEBUG_MSG("Joined collective");

    struct fi_rma_iov rma_iov;
    rma_iov.addr = (uint64_t)mr->start;
    rma_iov.key = fi_mr_key(mr->fid);

    struct fi_rma_iov *results = malloc(num_pes * sizeof(struct fi_rma_iov));
    if (results == NULL) {
        pthread_mutex_unlock(&rofi->dist->lock);
        ERR_MSG("malloc failed");
        return -1;
    }

    fi_addr_t coll_addr2 = fi_mc_addr(mc);
    ret = fi_allgather(rofi->dist->ep, &rma_iov, sizeof(rma_iov), NULL, results, NULL, coll_addr2, FI_UINT8, 0, &done_flag);
    if (ret) {
        ROFI_TRANSPORT_ERR_MSG("fi_allgather", ret);
        pthread_mutex_unlock(&rofi->dist->lock);
        free(results);
        return ret;
    }
    ret = rofi_transport_wait_on_context_comp(rofi->dist, &done_flag);
    if (ret) {
        ROFI_TRANSPORT_ERR_MSG("rofi_transport_wait_on_event", ret);
        pthread_mutex_unlock(&rofi->dist->lock);
        free(results);
        return ret;
    }

    for (int i = 0; i < num_pes; i++) {
        mr->iov[pes[i]] = results[i];
        DEBUG_MSG("i: %d(pe: %d), addr: 0x%lx, key: 0x%lx  ", i, pes[i], results[i].addr, results[i].key);
    }
    free(results);
    // for now we will immediately close down the collective group as we are currently only using them to do the addr+key transfer
    // in the future we probably want the collective group to persist as long as the memory region?
    DEBUG_MSG("Closing collective group");
    ret = fi_close(&mc->fid);
    if (ret) {
        ROFI_TRANSPORT_ERR_MSG("fi_close", ret);
        DEBUG_MSG("CLOSING AV SET");
        fi_close(&av_set->fid);
        if (ret) {
            ROFI_TRANSPORT_ERR_MSG("fi_close", ret);
        }
    }
    else {
        DEBUG_MSG("CLOSING AV SET");
        ret = fi_close(&av_set->fid);
        if (ret) {
            ROFI_TRANSPORT_ERR_MSG("fi_close", ret);
        }
    }

    pthread_mutex_unlock(&rofi->dist->lock);
    return ret;
}

int euclid_rem(int a, int b) {
    int r = a % b;
    return r >= 0 ? r : r + abs(b);
}

// This can support sub barriers
int rofi_transport_inner_barrier(rofi_transport_t *rofi, uint64_t *barrier_id, uint64_t *barrier_buf, uint64_t *pes, uint64_t me, uint64_t num_pes) {
    int n = 2;

    int num_rounds = ceil(log2((double)num_pes) / log2((double)n));

    int ret = 0;

    *barrier_id += 1;
    void *src = (void *)barrier_id;
    for (int round = 0; round < num_rounds; round++) {
        for (int i = 1; i <= n; i++) {
            int send_pe = euclid_rem((int)(me + i * pow(n + 1, round)), num_pes);
            send_pe = pes == NULL ? send_pe : pes[send_pe]; // if pes not null we are doing sub barrier

            // we need to store in the absolute pe location to prevent races.
            // allocations on multiple teams including the same PE can occur simultaneously,
            // the upper level runtime must ensure a given PE is only participating in one allocation at a time
            void *dst = (void *)(&barrier_buf[rofi->dist->desc.pe_id]);

            DEBUG_MSG("%d Sending %d to %d %p - %p + %p", me, *barrier_id, send_pe, dst, rofi->mr->start, rofi->mr->iov[send_pe].addr);
            struct fi_rma_iov rma_iov;
            rma_iov.addr = (uint64_t)(dst - rofi->mr->start + rofi->mr->iov[send_pe].addr);
            rma_iov.key = rofi->mr->iov[send_pe].key;
            DEBUG_MSG("%d Sending %d to %d %p", me, *barrier_id, send_pe, dst);
            ret = rofi_transport_put(rofi->dist, &rma_iov, send_pe, src, sizeof(uint64_t), rofi->mr->mr_desc, NULL);
            if (ret) {
                return ret;
            }
        }
        for (int i = 1; i <= n; i++) {
            int recv_pe = euclid_rem((int)(me - i * pow(n + 1, round)), num_pes);
            recv_pe = pes == NULL ? recv_pe : pes[recv_pe]; // if pes not null we are doing sub barrier
            DEBUG_MSG("%d Receiving %d from %d", me, *barrier_id, recv_pe);

            while (barrier_buf[recv_pe] < *barrier_id) {
                pthread_mutex_lock(&rofi->dist->lock);
                ret = rofi_transport_progress(rofi->dist);
                if (ret) {
                    return ret;
                }
                pthread_mutex_unlock(&rofi->dist->lock);
                sched_yield();
            }
        }
    }
    return 0;
}

int rofi_transport_barrier(rofi_transport_t *rofi) {
    return rofi_transport_inner_barrier(rofi, &rofi->global_barrier_id, rofi->global_barrier_buf, NULL, rofi->dist->desc.pe_id, rofi->dist->desc.pes);
}
