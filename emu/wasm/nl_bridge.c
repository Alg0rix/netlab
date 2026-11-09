/* netlab WASM bridge: shared-memory queues between Dynamips threads and JS.
 *
 * TX path (C -> JS): NIO send() and VTTY put_char push packed messages into
 * nlq_out; JS polls nl_drain()/nl_drain_buf() and copies them out.
 * RX path (JS -> C): JS writes into nl_inbuf() then calls nl_frame_rx() /
 * nl_con_in(), which enqueue into per-link rings drained by the NIO spec
 * threads and the console vtty buffer.
 *
 * Packed message format: u8 type | u8 link | u16 len | payload
 *   type 0 = ethernet frame, 1 = console bytes
 */
#include <pthread.h>
#include <stdint.h>
#include <string.h>
#include <stdlib.h>
#include <sys/types.h>
#include <emscripten.h>

struct virtual_tty;
typedef struct virtual_tty vtty_t;
int vtty_store_data(vtty_t *vtty,char *data,int len);

#define NLQ_CAP        (1 << 20)   /* 1 MiB packed output queue */
#define NL_NIO_MAX     128
#define NL_RXQ         64
#define NL_PKT_MAX     2048
#define NL_INBUF_SIZE  2048

#define NL_MSG_FRAME   0
#define NL_MSG_CONSOLE 1

static pthread_mutex_t nlq_lock = PTHREAD_MUTEX_INITIALIZER;
static uint8_t *nlq;               /* malloc'd at first push */
static int nlq_len;
static int nlq_tail = -1;          /* offset of last msg header, -1 if none */

static uint8_t nl_drain_out[NLQ_CAP];
static uint8_t nl_inbuf_data[NL_INBUF_SIZE];

/* ---------------- output queue ---------------- */

void nl_out_push(int type, int link, const void *data, int len)
{
   if (len <= 0 || len > NL_PKT_MAX)
      return;

   pthread_mutex_lock(&nlq_lock);
   if (!nlq)
      nlq = malloc(NLQ_CAP);
   if (nlq) {
      /* merge into the tail message when type+link match (console batching) */
      if (nlq_tail >= 0 && nlq[nlq_tail] == (uint8_t)type &&
          nlq[nlq_tail + 1] == (uint8_t)link)
      {
         int mlen = nlq[nlq_tail + 2] | (nlq[nlq_tail + 3] << 8);
         if (mlen + len <= NL_PKT_MAX) {
            memcpy(nlq + nlq_tail + 4 + mlen, data, (size_t)len);
            mlen += len;
            nlq[nlq_tail + 2] = (uint8_t)(mlen & 0xff);
            nlq[nlq_tail + 3] = (uint8_t)((mlen >> 8) & 0xff);
            nlq_len += len;
            pthread_mutex_unlock(&nlq_lock);
            return;
         }
      }
      if (nlq_len + 4 + len <= NLQ_CAP) {
         nlq_tail = nlq_len;
         nlq[nlq_len] = (uint8_t)type;
         nlq[nlq_len + 1] = (uint8_t)link;
         nlq[nlq_len + 2] = (uint8_t)(len & 0xff);
         nlq[nlq_len + 3] = (uint8_t)((len >> 8) & 0xff);
         memcpy(nlq + nlq_len + 4, data, (size_t)len);
         nlq_len += 4 + len;
      }
   }
   pthread_mutex_unlock(&nlq_lock);
}

EMSCRIPTEN_KEEPALIVE
int nl_drain(void)
{
   pthread_mutex_lock(&nlq_lock);
   int n = nlq_len;
   if (n > 0) {
      memcpy(nl_drain_out, nlq, (size_t)n);
      nlq_len = 0;
      nlq_tail = -1;
   }
   pthread_mutex_unlock(&nlq_lock);
   return n;
}

EMSCRIPTEN_KEEPALIVE
void *nl_drain_buf(void)
{
   return nl_drain_out;
}

EMSCRIPTEN_KEEPALIVE
void *nl_inbuf(void)
{
   return nl_inbuf_data;
}

/* ---------------- NIO desc ---------------- */

typedef struct {
   int link;
   int in_use;
   int closing;
   pthread_mutex_t lock;
   pthread_cond_t cond;
   uint8_t buf[NL_RXQ][NL_PKT_MAX];
   int len[NL_RXQ];
   int head;
   int count;
} nl_nio_t;

static nl_nio_t nl_nios[NL_NIO_MAX];

void *nl_nio_wasm_new(int link_id)
{
   int i;
   for (i = 0; i < NL_NIO_MAX; i++) {
      if (!nl_nios[i].in_use) {
         nl_nio_t *n = &nl_nios[i];
         memset(n, 0, sizeof(*n));
         n->link = link_id;
         n->in_use = 1;
         pthread_mutex_init(&n->lock, NULL);
         pthread_cond_init(&n->cond, NULL);
         return n;
      }
   }
   return NULL;
}

ssize_t nl_nio_wasm_send(void *dptr, void *pkt, size_t len)
{
   nl_nio_t *n = dptr;
   if (!n || n->closing)
      return -1;
   nl_out_push(NL_MSG_FRAME, n->link, pkt, (int)len);
   return (ssize_t)len;
}

ssize_t nl_nio_wasm_recv(void *dptr, void *pkt, size_t len)
{
   nl_nio_t *n = dptr;
   if (!n)
      return -1;

   pthread_mutex_lock(&n->lock);
   while (!n->count && !n->closing)
      pthread_cond_wait(&n->cond, &n->lock);
   if (n->closing || !n->count) {
      pthread_mutex_unlock(&n->lock);
      return -1;
   }
   int i = n->head;
   int l = n->len[i] < (int)len ? n->len[i] : (int)len;
   memcpy(pkt, n->buf[i], (size_t)l);
   n->head = (n->head + 1) % NL_RXQ;
   n->count--;
   pthread_mutex_unlock(&n->lock);
   return l;
}

void nl_nio_wasm_free(void *dptr)
{
   nl_nio_t *n = dptr;
   if (!n)
      return;
   pthread_mutex_lock(&n->lock);
   n->closing = 1;
   pthread_cond_broadcast(&n->cond);
   pthread_mutex_unlock(&n->lock);
   n->in_use = 0;
}

EMSCRIPTEN_KEEPALIVE
void nl_frame_rx(int link_id, int len)
{
   int i;
   if (len > NL_PKT_MAX)
      len = NL_PKT_MAX;
   for (i = 0; i < NL_NIO_MAX; i++) {
      nl_nio_t *n = &nl_nios[i];
      if (!n->in_use || n->link != link_id)
         continue;
      pthread_mutex_lock(&n->lock);
      if (n->count < NL_RXQ && !n->closing) {
         int pos = (n->head + n->count) % NL_RXQ;
         memcpy(n->buf[pos], nl_inbuf_data, (size_t)len);
         n->len[pos] = len;
         n->count++;
         pthread_cond_signal(&n->cond);
      }
      pthread_mutex_unlock(&n->lock);
      return;
   }
}

/* ---------------- console ---------------- */

static vtty_t *nl_con_vtty;

void nl_console_out(void *vtty, const void *buf, int len)
{
   if (!nl_con_vtty)
      nl_con_vtty = vtty;
   nl_out_push(NL_MSG_CONSOLE, 0, buf, len);
}

EMSCRIPTEN_KEEPALIVE
void nl_con_in(int len)
{
   if (!nl_con_vtty)
      return;
   if (len > NL_INBUF_SIZE)
      len = NL_INBUF_SIZE;
   vtty_store_data(nl_con_vtty, (char *)nl_inbuf_data, len);
}

/* ---- NVRAM snapshot (for browser persistence) ----------------------------
 * The nvram device is an mmap'd MEMFS file that only gets msync'd on clean
 * shutdown, which never happens when the worker is terminated. Expose the
 * live device memory instead; JS reads it straight out of HEAPU8.
 * Restore = write the file before callMain (memzone_create_file keeps
 * existing contents). */
#include "vm.h"
#include "device.h"

static int nl_nvram_size = 0;

EMSCRIPTEN_KEEPALIVE
int nl_nvram_ptr(void)
{
   vm_instance_t *vm = vm_acquire("default");
   int p = 0;
   nl_nvram_size = 0;
   if (vm) {
      struct vdevice *d = dev_get_by_name(vm, "nvram");
      if (d && d->host_addr) {
         p = (int)(uintptr_t)d->host_addr;
         nl_nvram_size = (int)d->phys_len;
      }
      vm_release(vm);
   }
   return p;
}

EMSCRIPTEN_KEEPALIVE
int nl_nvram_len(void) { return nl_nvram_size; }
