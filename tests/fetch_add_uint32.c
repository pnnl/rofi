#include <unistd.h>
#include <stdio.h>

#include "utils.h"
#include <rofi.h>

int main(void) {
    rofi_init(NULL, "verbs");
    unsigned int id = rofi_get_id();
    unsigned int size = rofi_get_size();

    if(id == 0) {
        rofi_banner("Atomic Fetch Add Test");
    }
    rofi_barrier();

    uint32_t *ptr = NULL;
    int ret = rofi_alloc(sizeof(uint32_t), 0, (void **)&ptr);
    if (ret) {
        printf("Error allocating memory\n");
        return -1;
    }

    if (id == 0) {
        *ptr = 0;
        printf("ID: %u/%u, ptr: %p, value: %u\n", id, size, ptr, *ptr);
    }
    rofi_barrier();

    rofi_atomic_fetch_add(ptr, 1, 0);

    if(id == 0) {
        rofi_verify(*ptr == 1);
    }
    
    rofi_release(ptr); 
    rofi_finit();

    return 0;
}