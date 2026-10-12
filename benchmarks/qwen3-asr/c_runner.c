/* Independent persistent benchmark of antirez/qwen-asr's unquantized CPU API.
 * Model BF16 weights; encoder F32; decoder BF16->F32 kernels. No Q8 CPU path.
 * Build against pinned upstream objects, not Meetily. No capture/network.
 */
#define _POSIX_C_SOURCE 200809L
#include "qwen_asr.h"
#include "qwen_asr_audio.h"
#include "qwen_asr_kernels.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <sys/resource.h>
static double now(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC,&t); return t.tv_sec+t.tv_nsec/1e9; }
static double started, first;
static void callback(const char *piece, void *unused) { (void)unused; if (first<0 && piece && piece[strspn(piece," \t\r\n")]) first=now()-started; }
static void string(const char *s) {
    putchar('"');
    for (const unsigned char *p=(const unsigned char*)s; *p; p++) {
        if (*p=='"'||*p=='\\') { putchar('\\'); putchar(*p); }
        else if (*p<32) printf("\\u%04x",*p); else putchar(*p);
    }
    putchar('"');
}
int main(int argc, char **argv) {
    if (argc!=6) { fprintf(stderr,"model threads offline|stream inputs.tsv repeats\n"); return 2; }
    const int threads=atoi(argv[2]), repeats=atoi(argv[5]);
    if (threads<1||threads>12||repeats<1||repeats>5 || (strcmp(argv[3],"offline") && strcmp(argv[3],"stream"))) return 2;
    qwen_set_threads(threads); qwen_verbose=0;
    double t=now(); qwen_ctx_t *ctx=qwen_load(argv[1]);
    if (!ctx) return 3;
    double load=now()-t;
    qwen_set_token_callback(ctx,callback,NULL);
    ctx->stream_chunk_sec=2.0; ctx->stream_unfixed_chunks=2;
    FILE *input=fopen(argv[4],"r"); if (!input) return 4;
    char *line=NULL; size_t capacity=0; int warmed=0;
    while (getline(&line,&capacity,input)>0) {
        line[strcspn(line,"\r\n")]=0;
        char *save=NULL; char *id=strtok_r(line,"\t",&save), *lang=strtok_r(NULL,"\t",&save), *path=strtok_r(NULL,"\t",&save);
        if (!id||!lang||!path) return 5;
        int n=0; float *samples=qwen_load_wav(path,&n);
        if (!samples||n<=0||n>60*16000||qwen_set_force_language(ctx,lang)) return 6;
        if (!warmed) { started=now(); first=-1; char *warm=qwen_transcribe_audio(ctx,samples,n); if(!warm) return 7; free(warm); warmed=1; }
        for (int rep=0;rep<repeats;rep++) {
            started=now(); first=-1;
            char *text=strcmp(argv[3],"stream")==0 ? qwen_transcribe_stream(ctx,samples,n) : qwen_transcribe_audio(ctx,samples,n);
            double elapsed=now()-started;
            struct rusage usage; getrusage(RUSAGE_SELF,&usage);
            printf("{\"id\":"); string(id); printf(",\"language\":"); string(lang);
            printf(",\"text\":"); string(text?text:"");
            printf(",\"status\":\"%s\",\"rep\":%d,\"elapsed_seconds\":%.9f,\"audio_seconds\":%.6f,\"first_token_seconds\":",text?"ok":"error",rep,elapsed,n/16000.0);
            if(first<0) printf("null"); else printf("%.9f",first);
            printf(",\"load_seconds\":%.9f,\"peak_rss_kib\":%ld,\"encode_seconds\":%.9f,\"decode_seconds\":%.9f,\"threads\":%d,\"precision\":\"unquantized-BF16-F32\",\"engine\":\"antirez-c\",\"mode\":\"%s\"}\n",load,usage.ru_maxrss,ctx->perf_encode_ms/1000,ctx->perf_decode_ms/1000,threads,argv[3]);
            fflush(stdout);
            if(!text) return 8;
            free(text);
        }
        free(samples);
    }
    free(line); fclose(input); qwen_free(ctx); return 0;
}
