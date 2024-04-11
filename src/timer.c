#include "timer.h"

#include <sys/time.h>


void init_timer(rofi_timer_t* timer, int num_ids){
    timer->start = malloc(sizeof(struct timeval)*num_ids);
    timer->elapsed = malloc(sizeof(double)*num_ids);
    timer-> names = malloc(sizeof(char*)*num_ids);
    timer->max_id = 0;
}
void start_timer(rofi_timer_t* timer, int id, char* name){
    clock_gettime(CLOCK_MONOTONIC_RAW, &timer->start[id]);
    timer->names[id] = name;
    if (id > timer->max_id) timer->max_id = id;
};
void stop_timer(rofi_timer_t* timer, int id){
    struct timeval end;
    clock_gettime(CLOCK_MONOTONIC_RAW, &end);
    timer->elapsed[id] = (end.tv_sec - timer->start[id].tv_sec) + (end.tv_nsec - timer->start[id].tv_nsec)/1000000000.0;
}
void print_timer(rofi_timer_t* timer){
    for (int i = 0; i <= timer->max_id; i++){
        printf("%s: %f\n", timer->names[i], timer->elapsed[i]);      
    }
}