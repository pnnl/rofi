#include <assert.h>
#include <inttypes.h>
#include <math.h>
#include <rdma/fi_rma.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <unistd.h>

#include <rofi.h>
#include <rofi_atomic.h>
#include <rofi_debug.h>
#include <rofi_internal.h>
#include <transport.h>

rofi_transport_t rofi;

void *rofi_get_remote_addr_internal(void *addr, unsigned int id) {
    rofi_mr_desc *el = mr_get(&rofi, addr);
    int ret = 0;

    if (!el) {
        ERR_MSG("MR not found for address %p", addr);
        return NULL;
    }

    DEBUG_MSG("\t Found MR [0x%lx - 0x%lx] Addr: %p Key: 0x%lx ", el->start, el->start + el->size, (void *)(addr - (uintptr_t)el->start + el->iov[id].addr), el->mr_key);
    return (void *)(addr - (uintptr_t)el->start + el->iov[id].addr);
}

void *rofi_get_local_addr_from_remote_addr_internal(void *addr, unsigned int id) {
    rofi_mr_desc *el = mr_get_from_remote(&rofi, addr, id);
    int ret = 0;

    if (!el) {
        ERR_MSG("MR not found for remote address %p", addr);
        return NULL;
    }

    DEBUG_MSG("\t Found MR [0x%lx - 0x%lx] Addr: %p Key: 0x%lx", el->start, el->start + el->size, (void *)(addr - el->iov[id].addr + (uintptr_t)el->start), el->mr_key);
    return (void *)(addr - el->iov[id].addr + (uintptr_t)el->start);
}

int rofi_wait_internal(void) {
    rofi_transport_put_wait_all(&rofi);
    rofi_transport_get_wait_all(&rofi);
    // rofi_transport_barrier(&rofi);
    return 0;
}

void *rofi_alloc_internal(size_t size, unsigned long flags) {

    rofi_mr_desc *mr = mr_add(&rofi, size, flags);
    if (!mr) {
        ERR_MSG("Error allocating memory for memory region descriptor. Aborting!");
        return NULL;
    }

    if (rofi_transport_exchange_mr_info(&rofi, mr)) {
        ERR_MSG("Error exchanging memory region info. Aborting!");
        return NULL;
    }

    return mr->start;
}

void *rofi_sub_alloc_internal(size_t size, unsigned long flags, uint64_t *pes, uint64_t num_pes) {
    rofi_mr_desc *mr = mr_add(&rofi, size, flags);
    if (!mr) {
        ERR_MSG("Error allocating memory for memory region descriptor. Aborting!");
        return NULL;
    }

    if (rofi_transport_sub_exchange_mr_info(&rofi, mr, pes, num_pes)) {
        ERR_MSG("Error exchanging memory region info. Aborting!");
        return NULL;
    }

    return mr->start;
}

int rofi_release_internal(void *addr) {
    return mr_rm(&rofi, addr);
}

int rofi_sub_release_internal(void *addr, uint64_t *pes, uint64_t num_pes) {
    return mr_rm(&rofi, addr);
}

unsigned int rofi_get_size_internal(void) {
    return rofi.desc.nodes;
}

unsigned int rofi_get_id_internal(void) {
    return rofi.desc.nid;
}

// NOTE this is needed to ensure progress for something like n-way dissemination barriers
// as recipients of RDMA Put still need to proress their completion queues for others to continue
int rofi_flush_internal(void) {
    // DEBUG_MSG("\t Flushing...");
    pthread_mutex_lock(&rofi.lock);
    rofi_transport_progress(&rofi);
    pthread_mutex_unlock(&rofi.lock);
    return 0;
}

void rofi_barrier_internal(void) {
    rofi_transport_barrier(&rofi);
}

int rofi_put_internal(void *dst, void *src, size_t size, unsigned int id, unsigned long flags) {
    rofi_mr_desc *el = mr_get(&rofi, dst);
    struct fi_rma_iov rma_iov;
    int ret = 0;

    if (!el) {
        ERR_MSG("\t No mr found for address %p on node %u", dst, id);
        return -1;
    }
    DEBUG_MSG("\t Found MR [0x%p - 0x%p] Key: 0x%lx", el->start, el->start + el->size, el->mr_key);

    rma_iov.addr = (uint64_t)(dst - el->start + el->iov[id].addr);
    rma_iov.key = el->iov[id].key;
    if (rma_iov.key == 0) {
        ERR_MSG("\t No Key found for address %p on node %u", dst, id);
        return -1;
    }
    DEBUG_MSG("\t Writing %lu bytes from %p to address 0x%lx at node %u with key 0x%lx (threshold %lu, in-flight msgs: %lu) sync: %d",
              size, src, rma_iov.addr, id, rma_iov.key, rofi.desc.inject_size,
              rofi.put_cntr,
              flags & ROFI_SYNC);

    if (flags & ROFI_SYNC) {
        if (rofi_transport_put(&rofi, &rma_iov, id, src, size, el->mr_desc, NULL)) {
            ERR_MSG("\t Error writing %lu bytes from %p to address 0x%lx at node %u with key 0x%lx",
                    size, src, rma_iov.addr, id, rma_iov.key);
            return -1;
        }
        if (rofi_transport_put_wait_all(&rofi)) {
            ERR_MSG("\t Error waiting for put");
            return -1;
        }
    }
    else {
        if (rofi_transport_put(&rofi, &rma_iov, id, src, size, el->mr_desc, NULL)) {
            ERR_MSG("\t Error writing %lu bytes from %p to address 0x%lx at node %u with key 0x%lx",
                    size, src, rma_iov.addr, id, rma_iov.key);
            return -1;
        }
    }
    DEBUG_MSG("\t Done writing %lu bytes from %p to address 0x%lx at node %u with key 0x%lx",
              size, src, rma_iov.addr, id, rma_iov.key);
    return 0;
}

int rofi_get_internal(void *dst, void *src, size_t size, unsigned int id, unsigned long flags) {
    rofi_mr_desc *el = mr_get(&rofi, src);
    struct fi_rma_iov rma_iov;
    int ret = 0;

    if (!el) {
        ERR_MSG("\t No mr found for address %p on node %u", src, id);
        return -1;
    }
    DEBUG_MSG("\t Found MR [0x%p - 0x%p] Key: 0x%lx", el->start, el->start + el->size, el->mr_key);

    rma_iov.addr = (uint64_t)(src - el->start + el->iov[id].addr);
    rma_iov.key = el->iov[id].key;
    if (rma_iov.key == 0) {
        ERR_MSG("\t No Key found for address %p on node %u", src, id);
        return -1;
    }
    DEBUG_MSG("\t Reading %lu bytes from address 0x%lx to %p at node %u with key 0x%lx (threshold %lu, in-flight msgs: %lu) sync: %d",
              size, rma_iov.addr, dst, id, rma_iov.key, rofi.desc.max_message_size,
              rofi.get_cntr,
              flags & ROFI_SYNC);

    if (flags & ROFI_SYNC) {

        if (rofi_transport_get(&rofi, &rma_iov, id, dst, size, el->mr_desc, NULL)) {
            ERR_MSG("\t Error reading %lu bytes from address 0x%lx to %p at node %u with key 0x%lx",
                    size, rma_iov.addr, src, id, rma_iov.key);
            return -1;
        }
        if (rofi_transport_get_wait_all(&rofi)) {
            ERR_MSG("\t Error waiting for get");
        }
    }
    else {
        if (rofi_transport_get(&rofi, &rma_iov, id, dst, size, el->mr_desc, NULL)) {
            ERR_MSG("\t Error reading %lu bytes from address 0x%lx to %p at node %u with key 0x%lx",
                    size, rma_iov.addr, src, id, rma_iov.key);
            return -1;
        }
    }
    DEBUG_MSG("\t Done reading %lu bytes from address 0x%lx to %p at node %u with key 0x%lx",
              size, rma_iov.addr, dst, id, rma_iov.key);
    return 0;
}

int rofi_send_internal(unsigned long id, void *buf, size_t size, unsigned long flags) {
}

int rofi_recv_internal(unsigned long id, void *buf, size_t size, unsigned long flags) {
}

int rofi_init_internal(char *prov) {
    if (!prov) {
        ERR_MSG("ROFI provider not specified. Currently ROFI only supports \"verbs\".");
    }
    pthread_rwlock_init(&rofi.mr_lock, NULL);
    pthread_mutex_init(&rofi.lock, NULL);
    int ret = 0;
    rofi.desc.PageSize = sysconf(_SC_PAGESIZE);

    ret = rt_init();
    if (ret) {
        ERR_MSG("Error initializing ROFI RT. Aborting.");
        goto err;
    }

    rofi.desc.nodes = rt_get_size();
    rofi.desc.nid = rt_get_rank();

    rofi.info = (struct fi_info *)calloc(1, sizeof(struct fi_info));
    if (!rofi.info) {
        ERR_MSG("Error allocating memory for rofi. Aborting.");
        ret = EXIT_FAILURE;
        goto err;
    }

    DEBUG_MSG("Initializing process %d/%d...", rofi.desc.nid, rofi.desc.nodes);

    struct fi_info *hints = fi_allocinfo();
    if (!hints) {
        return EXIT_FAILURE;
    }

    hints->caps = FI_RMA | FI_ATOMIC | FI_COLLECTIVE; // eventually want FI_ATOMIC
    hints->addr_format = FI_FORMAT_UNSPEC;
    hints->domain_attr->resource_mgmt = FI_RM_ENABLED;
    hints->domain_attr->threading = FI_THREAD_DOMAIN;
    hints->domain_attr->data_progress = FI_PROGRESS_MANUAL;
    // FI_MR_BASIC == FI_MR_ALLOCATED | FI_MR_PROV_KEY | FI_MR_VIRT_ADDR -- which is only mode supported by verbs
    hints->domain_attr->mr_mode = FI_MR_ALLOCATED | FI_MR_PROV_KEY | FI_MR_VIRT_ADDR;
    hints->mode = FI_CONTEXT;
    hints->ep_attr->type = FI_EP_RDM;
    hints->tx_attr->op_flags = FI_DELIVERY_COMPLETE; // maybe need to change this to FI_INJECT_COMPLETE or FI_TRANSMIT_COMPLETE

    if (prov) {
        hints->fabric_attr->prov_name = strdup(prov);
    }

    // this isn't really needed for verbs since it is a connected endpoint
    rofi.remote_addrs = (fi_addr_t *)malloc(rofi.desc.nodes * sizeof(fi_addr_t));
    if (!rofi.remote_addrs) {
        ERR_MSG("Error allocating memory for remote addresses. Aborting!");
        return -ENOMEM;
    }

    for (int i = 0; i < rofi.desc.nodes; i++) {
        rofi.remote_addrs[i] = i;
    }

    rofi_transport_init(hints, &rofi);

    mr_init();
    uint64_t global_barrier_size = rofi.desc.nodes * sizeof(uint64_t);
    int rofi_mr_size = global_barrier_size;

    rofi.mr = mr_add(&rofi, rofi_mr_size, 0);
    if (!rofi.mr) {
        ERR_MSG("Error allocating memory for memory region alloc buffer. Aborting!");
        return -ENOMEM;
    }

    ret = rofi_transport_exchange_mr_info(&rofi, rofi.mr);

    if (ret) {
        return ret;
    }

    rofi.global_barrier_id = 0;
    rofi.global_barrier_buf = (uint64_t *)rofi.mr->start;

    for (int i = 0; i < rofi.desc.nid; i++) {
        rofi.global_barrier_buf[i] = 0;
    }
    fi_freeinfo(hints);
    return 0;

err:
    rofi.desc.status = ROFI_STATUS_ERR;
    return -1;
}

int rofi_finit_internal(void) {
    DEBUG_MSG("rofi_finit_internal");
    rofi.desc.status = ROFI_STATUS_TERM;
    rofi_wait_internal();
    pthread_mutex_lock(&rofi.lock);
    rofi_transport_progress(&rofi);
    pthread_mutex_unlock(&rofi.lock);

    if (rofi.desc.nodes > 1) {
        rt_barrier();
    }
    mr_free(&rofi);

    pthread_mutex_lock(&rofi.lock);
    rofi_transport_fini(&rofi);
    pthread_mutex_unlock(&rofi.lock);

    return 0;
}
