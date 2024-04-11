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
#include <timer.h>

rofi_transport_t rofi;

void *rofi_get_remote_addr_internal(void *addr, unsigned int id) {
    rofi_mr_desc *el = mr_get(&rofi, addr);
    int ret = 0;

    if (!el) {
        ERR_MSG("MR not found for address %p", addr);
        return NULL;
    }

    if (rofi.shm != NULL && rt_get_node_rank(id) != -1) {
        int shm_id = rt_get_node_rank(id);
        DEBUG_MSG("\t Found MR [0x%lx - 0x%lx] Addr: %p Key: 0x%lx ", el->start, el->start + el->size, (void *)(addr - (uintptr_t)el->start + el->shm->iov[id].addr), el->shm->iov[id].key);
        return (void *)(addr - (uintptr_t)el->start + el->shm->iov[id].addr);
    }
    else {
        DEBUG_MSG("\t Found MR [0x%lx - 0x%lx] Addr: %p Key: 0x%lx ", el->start, el->start + el->size, (void *)(addr - (uintptr_t)el->start + el->dist->iov[id].addr), el->dist->iov[id].key);
        return (void *)(addr - (uintptr_t)el->start + el->dist->iov[id].addr);
    }
}

void *rofi_get_local_addr_from_remote_addr_internal(void *addr, unsigned int id) {
    rofi_mr_desc *el = mr_get_from_remote(&rofi, addr, id);
    int ret = 0;

    if (!el) {
        ERR_MSG("MR not found for remote address %p", addr);
        return NULL;
    }

    if (rofi.shm != NULL && rt_get_node_rank(id) != -1) {
        int shm_id = rt_get_node_rank(id);
        DEBUG_MSG("\t Found MR [0x%lx - 0x%lx] Addr: %p Key: 0x%lx", el->start, el->start + el->size, (void *)(addr - el->shm->iov[id].addr + (uintptr_t)el->start), el->shm->iov[id].key);
        return (void *)(addr - el->shm->iov[id].addr + (uintptr_t)el->start);
    }
    else {
        DEBUG_MSG("\t Found MR [0x%lx - 0x%lx] Addr: %p Key: 0x%lx", el->start, el->start + el->size, (void *)(addr - el->dist->iov[id].addr + (uintptr_t)el->start), el->dist->iov[id].key);
        return (void *)(addr - el->dist->iov[id].addr + (uintptr_t)el->start);
    }
}

int rofi_wait_internal(void) {
    rofi_transport_put_wait_all(rofi.dist);
    rofi_transport_get_wait_all(rofi.dist);
    if (rofi.shm) {
        rofi_transport_put_wait_all(rofi.shm);
        rofi_transport_get_wait_all(rofi.shm);
    }
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

    if (rofi.shm) {
        uint64_t *shm_pes = calloc(sizeof(uint64_t), rofi.num_shm_pes);
        for (int i = 0; i < rofi.pes; i++) {
            int shm_pe = rt_get_node_rank(i);
            if (shm_pe != -1) {
                shm_pes[shm_pe] = i;
            }
        }
        if (rofi_transport_sub_exchange_mr_info(&rofi, mr, shm_pes, rofi.num_shm_pes, 1)) {
            ERR_MSG("Error exchanging shm memory region info. Aborting!");
            return NULL;
        }
        free(shm_pes);
    }

    return mr->start;
}

void *rofi_sub_alloc_internal(size_t size, unsigned long flags, uint64_t *pes, uint64_t num_pes) {
    rofi_mr_desc *mr = mr_add(&rofi, size, flags);
    if (!mr) {
        ERR_MSG("Error allocating memory for memory region descriptor. Aborting!");
        return NULL;
    }

    if (rofi_transport_sub_exchange_mr_info(&rofi, mr, pes, num_pes, 0)) {
        ERR_MSG("Error exchanging memory region info. Aborting!");
        return NULL;
    }

    if (rofi.shm) {
        int num_shm_pes = 0;
        for (int i = 0; i < num_pes; i++) {
            if (rt_get_node_rank(pes[i]) != -1) {
                num_shm_pes++;
            }
        }
        if (num_shm_pes > 0) {
            uint64_t *shm_pes = calloc(sizeof(uint64_t), rofi.num_shm_pes);
            for (int i = 0; i < num_pes; i++) {
                int shm_pe = rt_get_node_rank(pes[i]);
                if (shm_pe != -1) {
                    shm_pes[shm_pe] = i;
                }
            }
            if (rofi_transport_sub_exchange_mr_info(&rofi, mr, shm_pes, num_shm_pes, 1)) {
                ERR_MSG("Error exchanging shm memory region info. Aborting!");
                return NULL;
            }
            free(shm_pes);
        }
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
    return rofi.dist->desc.pes;
}

unsigned int rofi_get_id_internal(void) {
    return rofi.dist->desc.pe_id;
}

// NOTE this is needed to ensure progress for something like n-way dissemination barriers
// as recipients of RDMA Put still need to proress their completion queues for others to continue
int rofi_flush_internal(void) {
    // DEBUG_MSG("\t Flushing...");
    pthread_mutex_lock(&rofi.dist->lock);
    rofi_transport_progress(rofi.dist);
    pthread_mutex_unlock(&rofi.dist->lock);
    if (rofi.shm) {
        pthread_mutex_lock(&rofi.shm->lock);
        rofi_transport_progress(rofi.shm);
        pthread_mutex_unlock(&rofi.shm->lock);
    }
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

    int shm_id = rt_get_node_rank(id);

    if (rofi.shm != NULL && shm_id != -1) {
        DEBUG_MSG("\t Found MR [0x%p - 0x%p] Shm Key: 0x%lx Dist Key: 0x%lx", el->start, el->start + el->size, el->shm->mr_key, el->dist->mr_key);
        rma_iov.addr = (uint64_t)(dst - el->start + el->shm->iov[shm_id].addr);
        rma_iov.key = el->shm->iov[shm_id].key;
        if (rma_iov.key == 0) {
            ERR_MSG("\t No Key found for address %p on node %u %d", dst, id, shm_id);
            return -1;
        }
        DEBUG_MSG("\t SHM: Writing %lu bytes from %p to address 0x%lx at shm node %u (%u) with key 0x%lx (threshold %lu, in-flight msgs: %lu) sync: %d",
                  size, src, rma_iov.addr, shm_id, id, rma_iov.key, rofi.shm->desc.inject_size,
                  rofi.shm->put_cntr,
                  flags & ROFI_SYNC);
    }
    else {
        DEBUG_MSG("\t Found MR [0x%p - 0x%p] Key: 0x%lx", el->start, el->start + el->size, el->dist->mr_key);

        rma_iov.addr = (uint64_t)(dst - el->start + el->dist->iov[id].addr);
        rma_iov.key = el->dist->iov[id].key;
        if (rma_iov.key == 0) {
            ERR_MSG("\t No Key found for address %p on node %u", dst, id);
            return -1;
        }
        DEBUG_MSG("\t DIST: Writing %lu bytes from %p to address 0x%lx at node %u with key 0x%lx (threshold %lu, in-flight msgs: %lu) sync: %d",
                  size, src, rma_iov.addr, id, rma_iov.key, rofi.dist->desc.inject_size,
                  rofi.dist->put_cntr,
                  flags & ROFI_SYNC);
    }

    if (flags & ROFI_SYNC) {
        if (rofi.shm != NULL && shm_id != -1) {
            if (rofi_transport_put(rofi.shm, &rma_iov, shm_id, src, size, el->shm->mr_desc, NULL)) {
                ERR_MSG("\t SHM: Error writing %lu bytes from %p to address 0x%lx at shm node %u (%u) with key 0x%lx",
                        size, src, rma_iov.addr, shm_id, id, rma_iov.key);
                return -1;
            }
            if (rofi_transport_put_wait_all(rofi.shm)) {
                ERR_MSG("\t SHM: Error waiting for put");
                return -1;
            }
        }
        else {
            if (rofi_transport_put(rofi.dist, &rma_iov, id, src, size, el->dist->mr_desc, NULL)) {
                ERR_MSG("\t DIST: Error writing %lu bytes from %p to address 0x%lx at node %u with key 0x%lx",
                        size, src, rma_iov.addr, id, rma_iov.key);
                return -1;
            }
            if (rofi_transport_put_wait_all(rofi.dist)) {
                ERR_MSG("\t DIST: Error waiting for put");
                return -1;
            }
        }
    }
    else {
        if (rofi.shm != NULL && shm_id != -1) {
            if (rofi_transport_put(rofi.shm, &rma_iov, shm_id, src, size, el->shm->mr_desc, NULL)) {
                ERR_MSG("\t SHM: Error writing %lu bytes from %p to address 0x%lx at node %u (%u) with key 0x%lx",
                        size, src, rma_iov.addr, shm_id, id, rma_iov.key);
                return -1;
            }
        }
        else {
            if (rofi_transport_put(rofi.dist, &rma_iov, id, src, size, el->dist->mr_desc, NULL)) {
                ERR_MSG("\t DIST: Error writing %lu bytes from %p to address 0x%lx at node %u with key 0x%lx",
                        size, src, rma_iov.addr, id, rma_iov.key);
                return -1;
            }
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

    int shm_id = rt_get_node_rank(id);

    if (rofi.shm != NULL && shm_id != -1) {
        DEBUG_MSG("\t Found MR [0x%p - 0x%p] Key: 0x%lx", el->start, el->start + el->size, el->shm->mr_key);

        rma_iov.addr = (uint64_t)(src - el->start + el->shm->iov[id].addr);
        rma_iov.key = el->shm->iov[id].key;
        if (rma_iov.key == 0) {
            ERR_MSG("\t No Key found for address %p on node %u %d", src, id, shm_id);
            return -1;
        }

        DEBUG_MSG("\t SHM: Reading %lu bytes from address 0x%lx to %p at node %u with key 0x%lx (threshold %lu, in-flight msgs: %lu) sync: %d",
                  size, rma_iov.addr, dst, id, rma_iov.key, rofi.shm->desc.max_message_size,
                  rofi.shm->get_cntr,
                  flags & ROFI_SYNC);
    }
    else {
        DEBUG_MSG("\t Found MR [0x%p - 0x%p] Key: 0x%lx", el->start, el->start + el->size, el->dist->mr_key);

        rma_iov.addr = (uint64_t)(src - el->start + el->dist->iov[id].addr);
        rma_iov.key = el->dist->iov[id].key;
        if (rma_iov.key == 0) {
            ERR_MSG("\t No Key found for address %p on node %u", src, id);
            return -1;
        }

        DEBUG_MSG("\t DIST: Reading %lu bytes from address 0x%lx to %p at node %u with key 0x%lx (threshold %lu, in-flight msgs: %lu) sync: %d",
                  size, rma_iov.addr, dst, id, rma_iov.key, rofi.dist->desc.max_message_size,
                  rofi.dist->get_cntr,
                  flags & ROFI_SYNC);
    }

    if (flags & ROFI_SYNC) {
        if (rofi.shm != NULL && shm_id != -1) {
            if (rofi_transport_get(rofi.shm, &rma_iov, id, dst, size, el->shm->mr_desc, NULL)) {
                ERR_MSG("\t SHM: Error reading %lu bytes from address 0x%lx to %p at node %u with key 0x%lx",
                        size, rma_iov.addr, src, id, rma_iov.key);
                return -1;
            }
            if (rofi_transport_get_wait_all(rofi.shm)) {
                ERR_MSG("\t SHM: Error waiting for get");
            }
        }
        else {
            if (rofi_transport_get(rofi.dist, &rma_iov, id, dst, size, el->dist->mr_desc, NULL)) {
                ERR_MSG("\t DIST: Error reading %lu bytes from address 0x%lx to %p at node %u with key 0x%lx",
                        size, rma_iov.addr, src, id, rma_iov.key);
                return -1;
            }
            if (rofi_transport_get_wait_all(rofi.dist)) {
                ERR_MSG("\t DIST: Error waiting for get");
            }
        }
    }
    else {
        if (rofi.shm != NULL && shm_id != -1) {
            if (rofi_transport_get(rofi.shm, &rma_iov, id, dst, size, el->shm->mr_desc, NULL)) {
                ERR_MSG("\t SHM: Error reading %lu bytes from address 0x%lx to %p at node %u with key 0x%lx",
                        size, rma_iov.addr, src, id, rma_iov.key);
                return -1;
            }
        }
        else {
            if (rofi_transport_get(rofi.dist, &rma_iov, id, dst, size, el->dist->mr_desc, NULL)) {
                ERR_MSG("\t DIST: Error reading %lu bytes from address 0x%lx to %p at node %u with key 0x%lx",
                        size, rma_iov.addr, src, id, rma_iov.key);
                return -1;
            }
        }
    }
    DEBUG_MSG("\t Done reading %lu bytes from address 0x%lx to %p at node %u with key 0x%lx",
              size, rma_iov.addr, dst, id, rma_iov.key);
    return 0;
}

int rofi_send_internal(unsigned int pe, void *buf, size_t size, unsigned long flags) {
    if (rofi_transport_send(rofi.dist, buf, size, pe)) {
        ERR_MSG("\t Error sending %lu bytes to node %u", size, pe);
        return -1;
    }
    return 0;
}

int rofi_recv_internal(void *buf, size_t size, unsigned long flags) {
    if (rofi_transport_recv(rofi.dist, buf, size)) {
        ERR_MSG("\t Error receiving %lu bytes", size);
        return -1;
    }
    return 0;
}

rofi_names_t *rofi_parse_names_internal(char *names_list) {
    char token = ';';
    int name_cnt = 0;
    for (int i = 0; i < strlen(names_list); i++) {
        if (names_list[i] == token) {
            name_cnt++;
        }
    }
    name_cnt += 1;
    char **name_strs = (char **)calloc(name_cnt, sizeof(char *));

    int p = 0;
    int i = 0;
    for (int k = 0; k < strlen(names_list); k++) {
        if (names_list[k] == token) {
            name_strs[p] = strndup(&names_list[i], k - i);
            p++;
            i = k + 1;
        }
    }
    name_strs[p] = strndup(&names_list[i], strlen(names_list) - i);
    rofi_names_t *names = (rofi_names_t *)calloc(1, sizeof(rofi_names_t));
    names->num = name_cnt;
    names->names = name_strs;
    return names;
}

int rofi_init_internal(char *provs, char *domains) {
    init_timer(&rofi_timer, 100);
    pthread_rwlock_init(&rofi.mr_lock, NULL);

    int ret = 0;

    rofi.PageSize = sysconf(_SC_PAGESIZE);

    ret = rt_init();
    if (ret) {
        ERR_MSG("Error initializing ROFI RT. Aborting.");
        goto err;
    }
    DEBUG_MSG("sizeof sub transport %d", sizeof(rofi_sub_transport_t));
    rofi_sub_transport_t *dist = calloc(1, sizeof(rofi_sub_transport_t));
    if (dist == NULL) {
        ERR_MSG("Error allocating ROFI sub transport. Aborting.");
        goto err;
    }
    rofi.dist = dist;
    pthread_mutex_init(&dist->lock, NULL);
    dist->info = NULL;
    dist->desc.pes = rt_get_size();
    dist->desc.pe_id = rt_get_rank();
    rofi.pes = rt_get_size();
    rofi.pe_id = rt_get_rank();

    char buffer[256]; // if we have more than 2^256 nodes, something is either seriously wrong or seriously awesome
    snprintf(buffer, sizeof(buffer), "%d", dist->desc.pes);
    setenv("FI_UNIVERSE_SIZE", buffer, 0);

    DEBUG_MSG("Initializing process %d/%d...", dist->desc.pe_id, dist->desc.pes);

    struct fi_info *hints = fi_allocinfo();
    if (!hints) {
        return EXIT_FAILURE;
    }

    hints->caps = FI_RMA | FI_ATOMIC | FI_MSG | FI_COLLECTIVE;
    hints->addr_format = FI_FORMAT_UNSPEC;
    hints->domain_attr->resource_mgmt = FI_RM_ENABLED;
    hints->domain_attr->threading = FI_THREAD_DOMAIN;
    hints->domain_attr->data_progress = FI_PROGRESS_MANUAL;
    hints->domain_attr->mr_mode = FI_MR_BASIC; // FI_MR_ALLOCATED | FI_MR_PROV_KEY | FI_MR_VIRT_ADDR; //we do FI_MR_BASIC because tcp will clear the individual flags thus would require us to make our mr offsets 0-based
    hints->mode = FI_CONTEXT;
    hints->ep_attr->type = FI_EP_RDM;
    hints->tx_attr->op_flags = FI_DELIVERY_COMPLETE; // maybe need to change this to FI_INJECT_COMPLETE or FI_TRANSMIT_COMPLETE

    struct fi_info *shm_hints = fi_dupinfo(hints);
    shm_hints->caps ^= FI_COLLECTIVE; // shm wont need collective

    rofi_names_t *prov_names = NULL;
    if (provs) {
        prov_names = rofi_parse_names_internal(provs);
    }

    rofi_names_t *domain_names = NULL;
    if (domains) {
        domain_names = rofi_parse_names_internal(domains);
    }

    // I think the endpoints we support are all connected so I'm not sure these are even used?
    dist->remote_addrs = (fi_addr_t *)malloc(dist->desc.pes * sizeof(fi_addr_t));
    if (!dist->remote_addrs) {
        ERR_MSG("Error allocating memory for remote addresses. Aborting!");
        return -ENOMEM;
    }
    for (int i = 0; i < dist->desc.pes; i++) {
        dist->remote_addrs[i] = i;
    }

    rofi_transport_init(hints, &rofi, dist, prov_names, domain_names);
    int num_pes_local = 0;
    for (int i = 0; i < dist->desc.pes; i++) {
        if (rt_get_node_rank(i) != -1) {
            num_pes_local++;
        }
    }
    rofi.num_shm_pes = num_pes_local;

    DEBUG_MSG("Num Pes Local=%d", num_pes_local);
    rofi_sub_transport_t *shm = NULL;
    if (num_pes_local > 1) {
        shm = calloc(1, sizeof(rofi_sub_transport_t));
        if (shm == NULL) {
            ERR_MSG("Error allocating ROFI sub transport. Aborting.");
            goto err;
        }
        rofi.shm = shm;
        pthread_mutex_init(&shm->lock, NULL);
        shm->info = NULL;
        shm->local = 1;
        shm->desc.pes = num_pes_local;                        // dist->desc.pes;     // num_pes_local;
        shm->desc.pe_id = rt_get_node_rank(dist->desc.pe_id); // dist->desc.pe_id; // rt_get_node_rank(dist->desc.pe_id);
        shm->remote_addrs = (fi_addr_t *)malloc(shm->desc.pes * sizeof(fi_addr_t));
        if (!shm->remote_addrs) {
            ERR_MSG("Error allocating memory for remote addresses. Aborting!");
            return -ENOMEM;
        }
        for (int i = 0; i < shm->desc.pes; i++) {
            shm->remote_addrs[i] = i;
        }
        DEBUG_MSG("Initing Shm");
        rofi_names_t shm_name = {.num = 1, .names = (char *[]){"shm"}};
        rofi_transport_init(shm_hints, &rofi, shm, &shm_name, NULL);
    }

    if (prov_names) {
        for (int i = 0; i < prov_names->num; i++) {
            free(prov_names->names[i]);
        }
        free(prov_names->names);
        free(prov_names);
    }

    if (domain_names) {
        for (int i = 0; i < domain_names->num; i++) {
            free(domain_names->names[i]);
        }
        free(domain_names->names);
        free(domain_names);
    }

    mr_init();
    uint64_t global_barrier_size = dist->desc.pes * sizeof(uint64_t);
    uint64_t sub_alloc_barrier_size = dist->desc.pes * sizeof(uint64_t);
    uint64_t sub_alloc_size = dist->desc.pes * sizeof(struct fi_rma_iov);
    int rofi_mr_size = global_barrier_size + sub_alloc_barrier_size + sub_alloc_size;

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
    rofi.sub_alloc_barrier_buf = (uint64_t *)(rofi.mr->start + global_barrier_size);
    rofi.sub_alloc_buf = (struct fi_rma_iov *)(rofi.mr->start + global_barrier_size + sub_alloc_barrier_size);

    for (int i = 0; i < dist->desc.pe_id; i++) {
        rofi.global_barrier_buf[i] = 0;
        rofi.sub_alloc_barrier_buf[i] = 0;
        rofi.sub_alloc_buf[i].key = 0;
        rofi.sub_alloc_buf[i].addr = 0;
    }
    fi_freeinfo(hints);
    fi_freeinfo(shm_hints);
    rofi_transport_barrier(&rofi);

    if (shm) {
        DEBUG_MSG("ABOUT TO EXCHANGE SHMEM KEYS");
        uint64_t *shm_pes = calloc(sizeof(uint64_t), num_pes_local);
        for (int i = 0; i < dist->desc.pes; i++) {
            int shm_pe = rt_get_node_rank(i);
            if (shm_pe != -1) {
                shm_pes[shm_pe] = i;
            }
        }
        ret = rofi_transport_sub_exchange_mr_info(&rofi, rofi.mr, shm_pes, num_pes_local, 1);
        if (ret) {
            return ret;
        }
        free(shm_pes);
    }

    DEBUG_MSG("rofi init!!");
    return 0;

err:
    rofi.status = ROFI_STATUS_ERR;
    return -1;
}

int rofi_finit_internal(void) {
    DEBUG_MSG("rofi_finit_internal");
    rofi.status = ROFI_STATUS_TERM;
    rofi_wait_internal();
    pthread_mutex_lock(&rofi.dist->lock);
    rofi_transport_progress(rofi.dist);
    pthread_mutex_unlock(&rofi.dist->lock);
    if (rofi.shm) {
        pthread_mutex_lock(&rofi.shm->lock);
        rofi_transport_progress(rofi.shm);
        pthread_mutex_unlock(&rofi.shm->lock);
    }

    if (rofi.dist->desc.pes > 1) {
        rt_barrier();
    }
    mr_free(&rofi);

    pthread_mutex_lock(&rofi.dist->lock);
    rofi_transport_fini(rofi.dist);
    pthread_mutex_unlock(&rofi.dist->lock);

    if (rofi.shm) {
        pthread_mutex_lock(&rofi.shm->lock);
        rofi_transport_fini(rofi.shm);
        pthread_mutex_unlock(&rofi.shm->lock);
    }

    return 0;
}
