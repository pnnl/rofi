

#ifndef TIMER_H
#define TIMER_H
#include <sys/time.h>

typedef struct {
    struct timeval* start;
    double* elapsed;
    char** names;
    int max_id;
}rofi_timer_t;

extern rofi_timer_t rofi_timer;

void init_timer(rofi_timer_t* timer,int num_ids);
void start_timer(rofi_timer_t* timer, int id);
void stop_timer(rofi_timer_t* timer, int id);
void print_timer(rofi_timer_t* timer);