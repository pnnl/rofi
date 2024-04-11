#include <assert.h>
#include <pthread.h>
#include <rdma/fi_rma.h>
#include <stdio.h>
#include <sys/mman.h>

#include <rofi_debug.h>
#include <rofi_internal.h>

#ifdef __APPLE__
#include <mach-o/getsect.h>
#else
/* Declare data_start and end as weak to avoid a linker error if the symbols
 * are not present.  During initialization we check if the symbols exist. */
#pragma weak __data_start
#pragma weak _end
extern int __data_start;
extern int _end;
#endif

rofi_mr_desc *mr_tab = NULL;
void *mr_next_addr;
uint64_t mr_next_key = 0;
void *ofi_data_base = NULL;
unsigned long ofi_data_length = 0;

extern struct fid_domain *domain;

int mr_init() {
    mr_tab = NULL;

#ifdef __APPLE__
    ofi_data_base = (void *)get_etext();
    ofi_data_length = get_end() - get_etext();
#else
    if (&__data_start == (int *)0 || &_end == (int *)0)
        ERR_MSG("Unable to locate symmetric data segment (%p, %p)\n",
                (void *)&__data_start, (void *)&_end);

    ofi_data_base = (void *)&__data_start;
    ofi_data_length = (long)((char *)&_end - (char *)&__data_start);
#endif
    mr_next_addr = (void *)(((unsigned long)ofi_data_base + ofi_data_length + GIGA) & ~(GIGA - 1));

    DEBUG_MSG("data base=%p length=%ld next address=%p", ofi_data_base, ofi_data_length, mr_next_addr);
    return 0;
}

rofi_mr_desc *mr_add(rofi_transport_t *rofi, size_t size, unsigned long mode) {
    rofi_mr_desc *el, *tmp, *shm_el;
    void *addr = NULL;
    int err;
    unsigned long PageSize = rofi->PageSize;

    el = calloc(1, sizeof(rofi_mr_desc));
    if (!(el)) {
        ERR_MSG("Error allocating memory for memory region descriptor. Aborting!");
        goto err;
    }

    el->dist = malloc(sizeof(rofi_mr_inner_desc));
    if (!(el->dist)) {
        ERR_MSG("Error allocating memory for inner memory region descriptor. Aborting!");
        goto err;
    }

    el->dist->iov = (struct fi_rma_iov *)calloc(1, rofi->dist->desc.pes * sizeof(struct fi_rma_iov));
    if (!(el->dist->iov)) {
        ERR_MSG("Error allocating memory for remote memory region keys. Aborting!");
        goto err_el;
    }
#ifdef _DEBUG
    for (int i = 0; i < rofi->dist->desc.pes; i++)
        DEBUG_MSG("\t Node: %o Key: 0x%lx Addr: 0x%lx", i, el->dist->iov[i].key, el->dist->iov[i].addr);
#endif

    if (rofi->shm) {
        el->shm = malloc(sizeof(rofi_mr_inner_desc));
        if (!(el->shm)) {
            ERR_MSG("Error allocating memory for inner memory region descriptor. Aborting!");
            goto err;
        }

        el->shm->iov = (struct fi_rma_iov *)calloc(1, rofi->shm->desc.pes * sizeof(struct fi_rma_iov));
        if (!(el->shm->iov)) {
            ERR_MSG("Error allocating memory for remote memory region keys. Aborting!");
            goto err_el;
        }
#ifdef _DEBUG
        for (int i = 0; i < rofi->shm->desc.pes; i++)
            DEBUG_MSG("\t SHM: Node: %o Key: 0x%lx Addr: 0x%lx", i, el->shm->iov[i].key, el->shm->iov[i].addr);
#endif
    }

    pthread_rwlock_wrlock(&rofi->mr_lock);
    HASH_FIND_PTR(mr_tab, &addr, tmp);
    if (tmp)
        goto err_lock;

    size = ((PageSize - 1) & size) ? ((size + PageSize) & ~(PageSize - 1)) : size;

    addr = mmap(mr_next_addr, size, PROT_READ | PROT_WRITE,
                MAP_ANON | MAP_PRIVATE, -1, 0);

    if (addr == MAP_FAILED) {
        perror("mmap");
        goto err_lock;
    }

    err = fi_mr_reg(rofi->dist->domain, addr, size,
                    FI_READ | FI_WRITE | FI_REMOTE_READ | FI_REMOTE_WRITE,
                    0, mr_next_key, 0x0,
                    &(el->dist->fid), &(el->dist->ctx));
    if (err != FI_SUCCESS) {
        ERR_MSG("Error creating OFI memroy region (%d). Aborting.", err);
        goto err_mmap;
    }
    assert(err == 0);

    el->dist->mr_key = fi_mr_key(el->dist->fid);
    el->dist->mr_desc = fi_mr_desc(el->dist->fid);

    if (rofi->shm) {
        err = fi_mr_reg(rofi->shm->domain, addr, size,
                        FI_READ | FI_WRITE | FI_REMOTE_READ | FI_REMOTE_WRITE,
                        0, mr_next_key, 0x0,
                        &(el->shm->fid), &(el->shm->ctx));
        if (err != FI_SUCCESS) {
            ERR_MSG("Error creating OFI memroy region (%d). Aborting.", err);
            goto err_mmap;
        }
        assert(err == 0);
        el->shm->mr_key = fi_mr_key(el->shm->fid);
        el->shm->mr_desc = fi_mr_desc(el->shm->fid);
    }

    el->start = addr;
    el->size = size;
    el->mode = mode;

    HASH_ADD_PTR(mr_tab, start, el);

    mr_next_addr += size;
    mr_next_key++;
    if (rofi->shm) {
        DEBUG_MSG("\t Added new memory region: ADDR=%p (%p) size=0x%lx mode=%u key=0x%lx desc=%p shm_key=0x%lx shm_desc=%p next=%p",
                  el->start, mr_next_addr, el->size, el->mode, el->dist->mr_key, el->dist->mr_desc, el->shm->mr_key, el->shm->mr_desc, mr_next_addr);
    }
    else {
        DEBUG_MSG("\t Added new memory region: ADDR=%p (%p) size=0x%lx mode=%u key=0x%lx desc=%p next=%p",
                  el->start, mr_next_addr, el->size, el->mode, el->dist->mr_key, el->dist->mr_desc, mr_next_addr);
    }

    pthread_rwlock_unlock(&rofi->mr_lock);
    return el;

err_mmap:
    munmap(addr, size);
err_lock:
    pthread_rwlock_unlock(&rofi->mr_lock);
err_iov:
    free(el->dist->iov);
    if (el->shm) {
        free(el->shm->iov);
    }
err_el:
    free(el->dist);
    if (el->shm) {
        free(el->shm);
    }
    free(el);

err:
    return NULL;
    ;
}

rofi_mr_desc *mr_get(rofi_transport_t *rofi, const void *addr) {
    rofi_mr_desc *el = NULL;
    rofi_mr_desc *tmp = NULL;

    pthread_rwlock_rdlock(&rofi->mr_lock);
    HASH_ITER(hh, mr_tab, el, tmp) {
        // DEBUG_MSG("\t MR %p - %p size=%ld mode=0x%x",
        //           el->start, el->start + el->size, el->size, el->mode);
        if (el->start <= addr && addr < (el->start + el->size))
            goto out;
    }
    el = NULL;

out:
    pthread_rwlock_unlock(&rofi->mr_lock);
    return el;
}

rofi_mr_desc *mr_get_from_remote(rofi_transport_t *rofi, const void *remote_addr, unsigned long remote_id) {
    rofi_mr_desc *el = NULL;
    rofi_mr_desc *tmp = NULL;

    pthread_rwlock_rdlock(&rofi->mr_lock);
    HASH_ITER(hh, mr_tab, el, tmp) {

        void *start = (void *)el->dist->iov[remote_id].addr; // we can always use dist
        void *end = start + el->size;
        // DEBUG_MSG("\t MR %p - %p size=%ld mode=0x%x remote address = %p",
        //   start, end, el->size, el->mode, remote_addr);
        if (start <= remote_addr && remote_addr < end) {
            // DEBUG_MSG("\t start <= remote_addr %d  remote_addr < end %d", start <= remote_addr, remote_addr < end);
            goto out;
        }
    }
    el == NULL;

out:
    pthread_rwlock_unlock(&rofi->mr_lock);
    return el;
}

int mr_rm(rofi_transport_t *rofi, void *addr) {
    rofi_mr_desc *el, *tmp;
    int ret = 0;

    pthread_rwlock_wrlock(&rofi->mr_lock);
    HASH_ITER(hh, mr_tab, el, tmp) {
        if (el->start == addr) {
            DEBUG_MSG("Removing memory region %p size 0x%lx", el->start, el->size);
            HASH_DEL(mr_tab, el);
            fi_close((struct fid *)el->dist->fid);
            if (el->shm) {
                fi_close((struct fid *)el->shm->fid);
                free(el->shm->iov);
                free(el->shm);
            }
            munmap(el->start, el->size);
            free(el->dist->iov);
            free(el->dist);
            free(el);
            goto out;
        }
    }
    ERR_MSG("Memory region %p not found!", addr);
    ret = -1;

out:
    pthread_rwlock_unlock(&rofi->mr_lock);
    return ret;
}

int mr_free(rofi_transport_t *rofi) {
    rofi_mr_desc *el, *tmp;

    pthread_rwlock_wrlock(&rofi->mr_lock);
    HASH_ITER(hh, mr_tab, el, tmp) {
        DEBUG_MSG("\t MR ADDR=%p size=%ld mode=0x%x",
                  el->start, el->size, el->mode);
        HASH_DEL(mr_tab, el);
        fi_close((struct fid *)el->dist->fid);
        if (el->shm) {
            fi_close((struct fid *)el->shm->fid);
        }
        munmap(el->start, el->size);
        free(el->dist->iov);
        free(el->dist);
        free(el);
    }
    pthread_rwlock_unlock(&rofi->mr_lock);
    return 0;
}
