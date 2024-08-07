#ifndef ROFI_H
#define ROFI_H
#include <stddef.h>
#include <stdint.h>

#define ROFI_ERR_ALLOC 0x01

int rofi_init(char *, char *);
int rofi_finit(void);
unsigned int rofi_get_size(void);
unsigned int rofi_get_id(void);
int rofi_flush(void);
int rofi_put(void *, void *, size_t, unsigned int, unsigned long);
int rofi_iput(void *, void *, size_t, unsigned int, unsigned long);
int rofi_get(void *, void *, size_t, unsigned int, unsigned long);
int rofi_iget(void *, void *, size_t, unsigned int, unsigned long);
int rofi_send(unsigned int, void *, size_t, unsigned long);
int rofi_recv(void *, size_t, unsigned long);
int rofi_alloc(size_t, unsigned long, void **);
int rofi_sub_alloc(size_t, unsigned long, void **, uint64_t *, uint64_t);
int rofi_release(void *);
int rofi_sub_release(void *, uint64_t *, uint64_t);
void rofi_barrier(void);
int rofi_wait(void);
void *rofi_get_remote_addr(void *, unsigned int);
void *rofi_get_local_addr_from_remote_addr(void *, unsigned int);
#endif
