/* Minimal libelf-compatible shim for the WASM build of dynamips.
 * Read-only ELF32 support with both byte orders (IOS images are MSB). */
#ifndef SHIM_LIBELF_H
#define SHIM_LIBELF_H

#include <stdint.h>
#include <stddef.h>

typedef struct Elf Elf;
typedef struct Elf_Scn Elf_Scn;

typedef uint16_t Elf32_Half;
typedef uint32_t Elf32_Word;
typedef int32_t  Elf32_Sword;
typedef uint64_t Elf32_Xword;
typedef int64_t  Elf32_Sxword;
typedef uint32_t Elf32_Addr;
typedef uint32_t Elf32_Off;
typedef uint16_t Elf32_Section;
typedef uint16_t Elf32_Versym;

#define EI_NIDENT 16

typedef struct {
    unsigned char e_ident[EI_NIDENT];
    Elf32_Half    e_type;
    Elf32_Half    e_machine;
    Elf32_Word    e_version;
    Elf32_Addr    e_entry;
    Elf32_Off     e_phoff;
    Elf32_Off     e_shoff;
    Elf32_Word    e_flags;
    Elf32_Half    e_ehsize;
    Elf32_Half    e_phentsize;
    Elf32_Half    e_phnum;
    Elf32_Half    e_shentsize;
    Elf32_Half    e_shnum;
    Elf32_Half    e_shstrndx;
} Elf32_Ehdr;

typedef struct {
    Elf32_Word   sh_name;
    Elf32_Word   sh_type;
    Elf32_Word   sh_flags;
    Elf32_Addr   sh_addr;
    Elf32_Off    sh_offset;
    Elf32_Word   sh_size;
    Elf32_Word   sh_link;
    Elf32_Word   sh_info;
    Elf32_Word   sh_addralign;
    Elf32_Word   sh_entsize;
} Elf32_Shdr;

typedef struct {
    Elf32_Word   p_type;
    Elf32_Off    p_offset;
    Elf32_Addr   p_vaddr;
    Elf32_Addr   p_paddr;
    Elf32_Word   p_filesz;
    Elf32_Word   p_memsz;
    Elf32_Word   p_flags;
    Elf32_Word   p_align;
} Elf32_Phdr;

typedef struct {
    Elf32_Word   st_name;
    Elf32_Addr   st_value;
    Elf32_Word   st_size;
    unsigned char st_info;
    unsigned char st_other;
    Elf32_Section st_shndx;
} Elf32_Sym;

/* e_ident indexes / values */
#define EI_MAG0   0
#define EI_MAG1   1
#define EI_MAG2   2
#define EI_MAG3   3
#define EI_CLASS  4
#define EI_DATA   5
#define ELFMAG0   0x7f
#define ELFMAG1   'E'
#define ELFMAG2   'L'
#define ELFMAG3   'F'
#define ELFCLASS32 1
#define ELFCLASS64 2
#define ELFDATA2LSB 1
#define ELFDATA2MSB 2

/* commands / versions */
#define ELF_C_NULL  0
#define ELF_C_READ  1
#define ELF_C_WRITE 2
#define ELF_C_RDWR  3
#define EV_NONE    0
#define EV_CURRENT 1

/* section types / flags */
#define SHT_NULL    0
#define SHT_PROGBITS 1
#define SHT_SYMTAB  2
#define SHT_STRTAB  3
#define SHT_NOBITS  8
#define SHF_WRITE    0x1
#define SHF_ALLOC    0x2
#define SHF_EXECINSTR 0x4

/* program header types */
#define PT_NULL 0
#define PT_LOAD 1

/* object types */
#define ET_NONE 0
#define ET_REL  1
#define ET_EXEC 2
#define ET_DYN  3

/* machine ids */
#define EM_NONE   0
#define EM_MIPS   8
#define EM_PPC    20
#define EM_SPARC32PLUS 18
#define EM_SPARCV9 43

#ifdef __cplusplus
extern "C" {
#endif

unsigned int elf_version(unsigned int ver);
Elf *elf_begin(int fd, int cmd, Elf *ref);
int elf_end(Elf *elf);
int elf_errno(void);
const char *elf_errmsg(int err);
Elf32_Ehdr *elf32_getehdr(Elf *elf);
Elf32_Phdr *elf32_getphdr(Elf *elf);
Elf_Scn *elf_getscn(Elf *elf, size_t index);
Elf32_Shdr *elf32_getshdr(Elf_Scn *scn);
char *elf_strptr(Elf *elf, size_t index, size_t offset);

#ifdef __cplusplus
}
#endif

#endif
