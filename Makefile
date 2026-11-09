.PHONY: wasm serve build emu

wasm:
	cd wasm && wasm-pack build --target web --out-dir ../web/pkg

serve:
	cd web && python3 serve.py 8471

# Copy built dynamips.js+wasm into web/ so the page can load them
emu-artifacts:
	cp emu/dynamips/build-wasm/stable/dynamips.js web/dynamips.js
	cp emu/dynamips/build-wasm/stable/dynamips.wasm web/dynamips.wasm

EMU_IMG := emscripten/emsdk:3.1.74
EMU_DIR := $(shell pwd)/emu

emu/dynamips/.git:
	./emu/fetch-dynamips.sh

# libelf shim for Dynamips' ELF loader (compiled with -pthread: the final link uses shared memory)
emu/shim/libelf.a: emu/shim/libelf.c emu/shim/libelf.h
	docker run --rm -v "$(EMU_DIR)":/src -w /src/shim $(EMU_IMG) bash -c "\
	  emcc -O2 -pthread -c libelf.c -o libelf.o && emar rcs libelf.a libelf.o"

# Build Dynamips -> WASM (dynamips.js + dynamips.wasm land in emu/dynamips/build-wasm/stable/)
emu: emu/dynamips/.git emu/shim/libelf.a
	docker run --rm -v "$(EMU_DIR)":/src -w /src/dynamips $(EMU_IMG) bash -c "\
	  mkdir -p build-wasm && cd build-wasm && \
	  emcmake cmake .. -DDYNAMIPS_ARCH=nojit -DDYNAMIPS_CODE=stable -DANY_COMPILER=1 \
	    -DCMAKE_BUILD_TYPE=Release \
	    -DLIBELF_INCLUDE_DIRS=/src/shim -DLIBELF_LIBRARIES=/src/shim/libelf.a \
	    -DCMAKE_C_FLAGS='-pthread' \
	    -DCMAKE_EXE_LINKER_FLAGS='-pthread -sPTHREAD_POOL_SIZE=16 -sPROXY_TO_PTHREAD \
	      -sALLOW_MEMORY_GROWTH=1 -sINITIAL_MEMORY=256MB \
	      -sEXPORTED_RUNTIME_METHODS=FS,HEAPU8,ccall,callMain \
	      -sMODULARIZE -sEXPORT_NAME=createIOS -sINVOKE_RUN=0 \
	      -sASSERTIONS=1 --pre-js /src/wasm/pre.js' && \
	  emmake make -j8 dynamips_nojit_stable"

build: wasm
