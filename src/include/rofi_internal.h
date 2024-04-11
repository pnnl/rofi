#ifndef ROFI_INTERNAL_H
#define ROFI_INTERNAL_H

#include <pthread.h>

#include <pthread.h>
#include <rdma/fabric.h>
#include <rdma/fi_cm.h>
#include <rdma/fi_domain.h>
#include <rdma/fi_endpoint.h>

#include <uthash.h>

#ifndef ROFI_FI_VERSION
#define ROFI_FI_VERSION FI_VERSION(1, 20)
#endif

typedef struct rofi_transport_t rofi_transport_t;

#include "context.h"
#include "mr.h"
#include "rt.h"

#define ROFI_STATUS_NONE 0
#define ROFI_STATUS_START 1
#define ROFI_STATUS_ACTIVE 2
#define ROFI_STATUS_TERM 3
#define ROFI_STATUS_END 4
#define ROFI_STATUS_ERR 5

#define ROFI_SERVER 1
#define ROFI_CLIENT 2

#define MAX_HOSTNAME_LEN 256
#define KILO 1024UL
#define MEGA 1024UL * KILO
#define GIGA 1024UL * MEGA

#define ROFI_HEAP_NOTALLOCATED 0
#define ROFI_HEAP_ALLOCATED 1
#define ROFI_HEAP_REGISTERED 2
#define ROFI_HEAP_INVALID 3

#define ROFI_ASYNC 0x1
#define ROFI_SYNC 0x2

typedef struct {
    unsigned int pes;
    unsigned int pe_id;
    int addrlen;
    uint64_t max_message_size;
    uint64_t inject_size;
} rofi_desc_t;

typedef struct rofi_prov_names_t {
    char **names;
    int num;
} rofi_names_t;

// typedef struct {
//     struct fid_fabric *fabric;
//     struct fid_domain *domain;
//     struct fid_eq *eq;
// } rofi_fab_resources_t;

// typedef struct {
//     struct fid_cntr *put_cntr;
//     struct fid_cntr *get_cntr;
//     struct fid_cntr *send_cntr;
//     struct fid_cntr *recv_cntr;
//     uint64_t pending_put_cntr;
//     uint64_t pending_get_cntr;
//     uint64_t pending_send_cntr;
//     uint64_t pending_recv_cntr;
// } rofi_counters_t;

// typedef struct {
//     struct fid_cq *cq;
//     struct fid_av *av;
//     struct fid_ep *ep;
//     fi_addr_t *remote_addrs;
// } rofi_ep_resources_t;
// typedef struct {
//     struct fi_info *info;
//     rofi_fab_resources_t fab_resources;
//     rofi_counters_t counters;
//     rofi_ep_resources_t ep_resources;
// } rofi_shm_transport_t;

typedef struct rofi_sub_transport_t {
    rofi_desc_t desc;
    struct fi_info *info;
    struct fid_fabric *fabric;
    struct fid_domain *domain;
    struct fid_eq *eq;
    struct fid_cq *cq;
    struct fid_av *av;
    struct fid_ep *ep;
    fi_addr_t *remote_addrs;
    struct fid_cntr *put_cntr;
    struct fid_cntr *get_cntr;
    struct fid_cntr *send_cntr;
    struct fid_cntr *recv_cntr;
    uint64_t pending_put_cntr;
    uint64_t pending_get_cntr;
    uint64_t pending_send_cntr;
    uint64_t pending_recv_cntr;
    pthread_mutex_t lock;
    uint64_t fi_collective;
    int local;
} rofi_sub_transport_t;

struct rofi_transport_t {
    unsigned int pes;
    unsigned int pe_id;
    unsigned int num_shm_pes;
    unsigned long status;
    struct rofi_sub_transport_t *dist;
    struct rofi_sub_transport_t *shm;
    unsigned long PageSize;
    rofi_mr_desc *mr;
    uint64_t global_barrier_id;
    uint64_t *global_barrier_buf;
    uint64_t *sub_alloc_barrier_buf;
    struct fi_rma_iov *sub_alloc_buf;
    pthread_rwlock_t mr_lock;
};

extern rofi_transport_t rofi;

int rofi_init_internal(char *, char *);
int rofi_finit_internal(void);
unsigned int rofi_get_size_internal(void);
unsigned int rofi_get_id_internal(void);
int rofi_flush_internal(void);
void rofi_barrier_internal(void);
int rofi_put_internal(void *, void *, size_t, unsigned int, unsigned long);
int rofi_get_internal(void *, void *, size_t, unsigned int, unsigned long);
int rofi_send_internal(unsigned int, void *, size_t, unsigned long);
int rofi_recv_internal(void *, size_t, unsigned long);
void *rofi_alloc_internal(size_t, unsigned long);
void *rofi_sub_alloc_internal(size_t, unsigned long, uint64_t *, uint64_t);
int rofi_release_internal(void *);
int rofi_sub_release_internal(void *, uint64_t *, uint64_t);
int rofi_wait_internal(void);
void *rofi_get_remote_addr_internal(void *, unsigned int);
void *rofi_get_local_addr_from_remote_addr_internal(void *, unsigned int);

#endif
