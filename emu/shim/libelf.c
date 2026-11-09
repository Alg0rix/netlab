/* Minimal libelf-compatible implementation for the dynamips WASM build.
 * Read-only ELF32; converts multi-byte fields when the file byte order
 * differs from the host (IOS MIPS/PPC images are big-endian). */
#include "libelf.h"

#include <stdlib.h>
#include <string.h>
#include <unistd.h>

struct Elf_Scn {
    Elf *elf;
    size_t index;
};

struct Elf {
    unsigned char *data;
    size_t size;
    int msb;              /* file is big-endian */
    Elf32_Ehdr ehdr;      /* host-order copy */
    Elf32_Shdr *shdrs;    /* host-order copies, e_shnum entries */
    Elf32_Phdr *phdrs;
    Elf_Scn *scns;
};

static int elf_err_no;

static uint16_t rd16(const struct Elf *e, uint16_t v) {
    if (!e->msb) return v;
    return (uint16_t)((v >> 8) | (v << 8));
}

static uint32_t rd32(const struct Elf *e, uint32_t v) {
    if (!e->msb) return v;
    return ((v & 0xff) << 24) | ((v & 0xff00) << 8) |
           ((v >> 8) & 0xff00) | ((v >> 24) & 0xff);
}

unsigned int elf_version(unsigned int ver) {
    (void)ver;
    return EV_CURRENT;
}

static int read_all(int fd, unsigned char **out, size_t *out_len) {
    size_t cap = 1 << 20, len = 0;
    unsigned char *buf = malloc(cap);
    if (!buf) return -1;
    for (;;) {
        if (len == cap) {
            cap *= 2;
            buf = realloc(buf, cap);
            if (!buf) return -1;
        }
        ssize_t n = read(fd, buf + len, cap - len);
        if (n < 0) { free(buf); return -1; }
        if (n == 0) break;
        len += (size_t)n;
    }
    *out = buf;
    *out_len = len;
    return 0;
}

Elf *elf_begin(int fd, int cmd, Elf *ref) {
    (void)ref;
    elf_err_no = 0;
    if (cmd != ELF_C_READ) { elf_err_no = 1; return NULL; }

    Elf *e = calloc(1, sizeof(Elf));
    if (!e) { elf_err_no = 2; return NULL; }
    if (read_all(fd, &e->data, &e->size) != 0) { free(e); elf_err_no = 3; return NULL; }
    if (e->size < sizeof(Elf32_Ehdr)) { elf_end(e); elf_err_no = 4; return NULL; }

    const unsigned char *h = e->data;
    if (h[EI_MAG0] != ELFMAG0 || h[EI_MAG1] != ELFMAG1 ||
        h[EI_MAG2] != ELFMAG2 || h[EI_MAG3] != ELFMAG3 ||
        h[EI_CLASS] != ELFCLASS32) {
        elf_end(e);
        elf_err_no = 4;
        return NULL;
    }
    e->msb = (h[EI_DATA] == ELFDATA2MSB);

    const Elf32_Ehdr *f = (const Elf32_Ehdr *)h;
    e->ehdr.e_type      = rd16(e, f->e_type);
    e->ehdr.e_machine   = rd16(e, f->e_machine);
    e->ehdr.e_version   = rd32(e, f->e_version);
    e->ehdr.e_entry     = rd32(e, f->e_entry);
    e->ehdr.e_phoff     = rd32(e, f->e_phoff);
    e->ehdr.e_shoff     = rd32(e, f->e_shoff);
    e->ehdr.e_flags     = rd32(e, f->e_flags);
    e->ehdr.e_ehsize    = rd16(e, f->e_ehsize);
    e->ehdr.e_phentsize = rd16(e, f->e_phentsize);
    e->ehdr.e_phnum     = rd16(e, f->e_phnum);
    e->ehdr.e_shentsize = rd16(e, f->e_shentsize);
    e->ehdr.e_shnum     = rd16(e, f->e_shnum);
    e->ehdr.e_shstrndx  = rd16(e, f->e_shstrndx);
    memcpy(e->ehdr.e_ident, f->e_ident, EI_NIDENT);

    /* section headers */
    if (e->ehdr.e_shoff && e->ehdr.e_shnum) {
        size_t n = e->ehdr.e_shnum;
        e->shdrs = calloc(n, sizeof(Elf32_Shdr));
        e->scns  = calloc(n, sizeof(Elf_Scn));
        if (!e->shdrs || !e->scns) { elf_end(e); elf_err_no = 2; return NULL; }
        const Elf32_Shdr *fs = (const Elf32_Shdr *)(h + e->ehdr.e_shoff);
        if ((size_t)(e->ehdr.e_shoff + n * sizeof(Elf32_Shdr)) > e->size) {
            elf_end(e); elf_err_no = 4; return NULL;
        }
        for (size_t i = 0; i < n; i++) {
            e->shdrs[i].sh_name      = rd32(e, fs[i].sh_name);
            e->shdrs[i].sh_type      = rd32(e, fs[i].sh_type);
            e->shdrs[i].sh_flags     = rd32(e, fs[i].sh_flags);
            e->shdrs[i].sh_addr      = rd32(e, fs[i].sh_addr);
            e->shdrs[i].sh_offset    = rd32(e, fs[i].sh_offset);
            e->shdrs[i].sh_size      = rd32(e, fs[i].sh_size);
            e->shdrs[i].sh_link      = rd32(e, fs[i].sh_link);
            e->shdrs[i].sh_info      = rd32(e, fs[i].sh_info);
            e->shdrs[i].sh_addralign = rd32(e, fs[i].sh_addralign);
            e->shdrs[i].sh_entsize   = rd32(e, fs[i].sh_entsize);
            e->scns[i].elf = e;
            e->scns[i].index = i;
        }
    }

    /* program headers */
    if (e->ehdr.e_phoff && e->ehdr.e_phnum) {
        size_t n = e->ehdr.e_phnum;
        e->phdrs = calloc(n, sizeof(Elf32_Phdr));
        if (!e->phdrs) { elf_end(e); elf_err_no = 2; return NULL; }
        const Elf32_Phdr *fp = (const Elf32_Phdr *)(h + e->ehdr.e_phoff);
        if ((size_t)(e->ehdr.e_phoff + n * sizeof(Elf32_Phdr)) > e->size) {
            elf_end(e); elf_err_no = 4; return NULL;
        }
        for (size_t i = 0; i < n; i++) {
            e->phdrs[i].p_type   = rd32(e, fp[i].p_type);
            e->phdrs[i].p_offset = rd32(e, fp[i].p_offset);
            e->phdrs[i].p_vaddr  = rd32(e, fp[i].p_vaddr);
            e->phdrs[i].p_paddr  = rd32(e, fp[i].p_paddr);
            e->phdrs[i].p_filesz = rd32(e, fp[i].p_filesz);
            e->phdrs[i].p_memsz  = rd32(e, fp[i].p_memsz);
            e->phdrs[i].p_flags  = rd32(e, fp[i].p_flags);
            e->phdrs[i].p_align  = rd32(e, fp[i].p_align);
        }
    }
    return e;
}

int elf_end(Elf *e) {
    if (!e) return 0;
    free(e->data);
    free(e->shdrs);
    free(e->phdrs);
    free(e->scns);
    free(e);
    return 0;
}

int elf_errno(void) {
    int v = elf_err_no;
    elf_err_no = 0;
    return v;
}

const char *elf_errmsg(int err) {
    switch (err) {
        case 0: return "no error";
        case 1: return "unsupported elf command";
        case 2: return "out of memory";
        case 3: return "read failed";
        case 4: return "invalid ELF file";
        default: return "unknown error";
    }
}

Elf32_Ehdr *elf32_getehdr(Elf *e) { return e ? &e->ehdr : NULL; }
Elf32_Phdr *elf32_getphdr(Elf *e) { return e ? e->phdrs : NULL; }

Elf_Scn *elf_getscn(Elf *e, size_t index) {
    if (!e || !e->scns || index >= e->ehdr.e_shnum) return NULL;
    return &e->scns[index];
}

Elf32_Shdr *elf32_getshdr(Elf_Scn *scn) {
    if (!scn || !scn->elf) return NULL;
    return &scn->elf->shdrs[scn->index];
}

char *elf_strptr(Elf *e, size_t index, size_t offset) {
    if (!e || index >= e->ehdr.e_shnum) return NULL;
    Elf32_Shdr *s = &e->shdrs[index];
    if (s->sh_type == SHT_NOBITS) return NULL;
    size_t off = s->sh_offset + offset;
    if (off >= e->size) return NULL;
    return (char *)e->data + off;
}
