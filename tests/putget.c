#include <unistd.h>
#include <stdio.h>
#include <string.h>
#include <time.h>
#include <math.h>

#include <rofi.h>
#include "utils.h"

#define N (1UL<<30)
//#define VERBOSE

typedef struct{
	unsigned long size;
	struct timespec start, end;
	double time;
	double tput;
} results_t;

static inline int verify_data(char* in, char* out, unsigned long size)
{
	unsigned long i;

	for(i=0; i<size; i++){
#ifdef VERBOSE
		printf("%d: %c\n", i, out[i]);
#endif
		if(in[i] != out[i]){
			printf("ERROR %lu: %c != %c\n", i, in[i], out[i]);
			return -1;
		}
	}

	return 0;
}

int main(void)
{
	unsigned int i;
	int ret = 0, err = 0;
	char* src;
	char* target;
	struct timespec start, end;
	char test_name[128];
	unsigned long size = 2;
	unsigned long ntests; 
	results_t* data;
	unsigned int  me, np;
	unsigned int ptest;

#ifdef ROFI_IPUT      
	strcpy(test_name, "ROFI iPut Test");
	ptest = 0;
#elif ROFI_IGET
	strcpy(test_name, "ROFI iGet Test");
	ptest = 1;
#endif
	
	ntests = (unsigned long) log2(N); 
	data = (results_t*) malloc (ntests * sizeof(results_t));
	if(!data){
		printf("Error allocating memory to store results. Aborting.\n");
		exit(EXIT_FAILURE);
	}
	
	rofi_init("verbs");
	np = rofi_get_size();
	if(np != 2){
		printf("Invalid number of processes (%u) (Required 2)! Aborting.\n", np);
		ret = -1;
		goto out;
	}
	
	me = rofi_get_id();

	if(me == ptest)
		rofi_banner(test_name);

	ret = rofi_alloc(2 * N, 0x0, (void**) &src);
	if(ret){
		printf("Error allocating ROFI heap");
		goto out;
	}
	
	for(i=0; i<N; i++)
		src[i] = 'a';
	
	target = src + N;
	
	for(size = 2, i = 0; size <= N; size *= 2, i++){
		memset( (void*) target, 0, N);
		
		rofi_barrier();
		
		if(me){
			clock_gettime(CLOCK_MONOTONIC, &(data[i].start));
#ifdef ROFI_IPUT
			if(rofi_iput(target, src, size, 0, 0x0)){
				printf("[%u] Error writing to remote node. Aborting...\n", me);
			}
#elif ROFI_IGET
			if(rofi_iget(target, src, size, 0, 0x0)){
				printf("[%u] Error reading from remote node. Aborting...\n", me);
			}
#endif
			clock_gettime(CLOCK_MONOTONIC, &(data[i].end));
		}
		
		rofi_barrier();
		
		if(me == ptest)
			err += verify_data(src, target, size);
		
		if(me){
			data[i].size = size;
			data[i].time = ((double) tdiff(data[i].end,data[i].start))/BILLION;
			data[i].tput = ((double) data[i].size) / data[i].time;
		}
	}	

	rofi_barrier();

	if(me){
		printf("\t %-10s \t %-11s \t %-19s\n", "Size (bytes)", "Time (sec)", "Throughput (bytes/sec)");
		for(i = 0; i < ntests; i++)
			printf("\t %10lu \t %06.4f \t %16.2f\n",
			       data[i].size, data[i].time, data[i].tput);
	}
	rofi_barrier();

	if(me == ptest)
		rofi_verify(err);

	rofi_release(src);
 out:
	rofi_finit();
	free(data);
	return 0;
}