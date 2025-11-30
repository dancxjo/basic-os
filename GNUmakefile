# Nuke built-in rules and variables.
MAKEFLAGS += -rR
.SUFFIXES:

# Convenience macro to reliably declare user overridable variables.
override USER_VARIABLE = $(if $(filter $(origin $(1)),default undefined),$(eval override $(1) := $(2)))

$(call USER_VARIABLE,MODE,native)

# Target architecture to build for. Default to x86_64.
$(call USER_VARIABLE,KARCH,x86_64)

ifeq ($(KARCH),x86_64)
    override RUST_TARGET := x86_64-unknown-none
else ifeq ($(KARCH),aarch64)
    override RUST_TARGET := aarch64-unknown-none
else ifeq ($(KARCH),riscv64)
    override RUST_TARGET := riscv64gc-unknown-none-elf
else ifeq ($(KARCH),loongarch64)
    override RUST_TARGET := loongarch64-unknown-none
endif

# Default user QEMU flags. These are appended to the QEMU command calls.
$(call USER_VARIABLE,QEMUFLAGS,-m 4G -serial mon\:stdio)
# $(call USER_VARIABLE,QEMUFLAGS,-m 4G)

override IMAGE_NAME := template-$(KARCH)

$(call USER_VARIABLE,HOST_NEO4J_URI,bolt://127.0.0.1:7687)
$(call USER_VARIABLE,HOST_NEO4J_USER,neo4j)
# Default password must meet Neo4j minimum length (>=8). Can be overridden on the command line.
$(call USER_VARIABLE,HOST_NEO4J_PASSWORD,neo4jpass)

.PHONY: all
all: $(IMAGE_NAME).iso

.PHONY: all-hdd
all-hdd: $(IMAGE_NAME).hdd

.PHONY: run
run: run-$(KARCH)

.PHONY: run-debug
run-debug: run-debug-$(KARCH)

.PHONY: run-hdd
run-hdd: run-hdd-$(KARCH)

.PHONY: run-x86_64
run-x86_64: ovmf/ovmf-code-$(KARCH).fd ovmf/ovmf-vars-$(KARCH).fd $(IMAGE_NAME).iso
	qemu-system-$(KARCH) \
	        -M q35 \
	        -drive if=pflash,unit=0,format=raw,file=ovmf/ovmf-code-$(KARCH).fd,readonly=on \
	        -drive if=pflash,unit=1,format=raw,file=ovmf/ovmf-vars-$(KARCH).fd \
	        -cdrom $(IMAGE_NAME).iso \
	        $(QEMUFLAGS)

.PHONY: run-debug-x86_64
run-debug-x86_64: ovmf/ovmf-code-$(KARCH).fd ovmf/ovmf-vars-$(KARCH).fd $(IMAGE_NAME).iso
	qemu-system-$(KARCH) \
	        -M q35 \
	        -drive if=pflash,unit=0,format=raw,file=ovmf/ovmf-code-$(KARCH).fd,readonly=on \
	        -drive if=pflash,unit=1,format=raw,file=ovmf/ovmf-vars-$(KARCH).fd \
	        -cdrom $(IMAGE_NAME).iso \
	        -S -s \
	        $(QEMUFLAGS)

.PHONY: run-hdd-x86_64
run-hdd-x86_64: ovmf/ovmf-code-$(KARCH).fd ovmf/ovmf-vars-$(KARCH).fd $(IMAGE_NAME).hdd
	qemu-system-$(KARCH) \
		-M q35 \
		-drive if=pflash,unit=0,format=raw,file=ovmf/ovmf-code-$(KARCH).fd,readonly=on \
		-drive if=pflash,unit=1,format=raw,file=ovmf/ovmf-vars-$(KARCH).fd \
		-hda $(IMAGE_NAME).hdd \
		$(QEMUFLAGS)

.PHONY: run-aarch64
run-aarch64: ovmf/ovmf-code-$(KARCH).fd ovmf/ovmf-vars-$(KARCH).fd $(IMAGE_NAME).iso
	qemu-system-$(KARCH) \
	        -M virt \
		-cpu cortex-a72 \
		-device ramfb \
		-device qemu-xhci \
		-device usb-kbd \
		-device usb-mouse \
		-drive if=pflash,unit=0,format=raw,file=ovmf/ovmf-code-$(KARCH).fd,readonly=on \
		-drive if=pflash,unit=1,format=raw,file=ovmf/ovmf-vars-$(KARCH).fd \
	        -cdrom $(IMAGE_NAME).iso \
	        $(QEMUFLAGS)

.PHONY: run-debug-aarch64
run-debug-aarch64: ovmf/ovmf-code-$(KARCH).fd ovmf/ovmf-vars-$(KARCH).fd $(IMAGE_NAME).iso
	qemu-system-$(KARCH) \
	        -M virt \
	        -cpu cortex-a72 \
	        -device ramfb \
	        -device qemu-xhci \
	        -device usb-kbd \
	        -device usb-mouse \
	        -drive if=pflash,unit=0,format=raw,file=ovmf/ovmf-code-$(KARCH).fd,readonly=on \
	        -drive if=pflash,unit=1,format=raw,file=ovmf/ovmf-vars-$(KARCH).fd \
	        -cdrom $(IMAGE_NAME).iso \
	        -S -s \
	        $(QEMUFLAGS)

.PHONY: run-hdd-aarch64
run-hdd-aarch64: ovmf/ovmf-code-$(KARCH).fd ovmf/ovmf-vars-$(KARCH).fd $(IMAGE_NAME).hdd
	qemu-system-$(KARCH) \
		-M virt \
		-cpu cortex-a72 \
		-device ramfb \
		-device qemu-xhci \
		-device usb-kbd \
		-device usb-mouse \
		-drive if=pflash,unit=0,format=raw,file=ovmf/ovmf-code-$(KARCH).fd,readonly=on \
		-drive if=pflash,unit=1,format=raw,file=ovmf/ovmf-vars-$(KARCH).fd \
		-hda $(IMAGE_NAME).hdd \
		$(QEMUFLAGS)

.PHONY: run-riscv64
run-riscv64: ovmf/ovmf-code-$(KARCH).fd ovmf/ovmf-vars-$(KARCH).fd $(IMAGE_NAME).iso
	qemu-system-$(KARCH) \
	        -M virt \
	        -cpu rv64 \
		-device ramfb \
		-device qemu-xhci \
		-device usb-kbd \
		-device usb-mouse \
		-drive if=pflash,unit=0,format=raw,file=ovmf/ovmf-code-$(KARCH).fd,readonly=on \
		-drive if=pflash,unit=1,format=raw,file=ovmf/ovmf-vars-$(KARCH).fd \
	        -cdrom $(IMAGE_NAME).iso \
	        $(QEMUFLAGS)

.PHONY: run-debug-riscv64
run-debug-riscv64: ovmf/ovmf-code-$(KARCH).fd ovmf/ovmf-vars-$(KARCH).fd $(IMAGE_NAME).iso
	qemu-system-$(KARCH) \
	        -M virt \
	        -cpu rv64 \
	        -device ramfb \
	        -device qemu-xhci \
	        -device usb-kbd \
	        -device usb-mouse \
	        -drive if=pflash,unit=0,format=raw,file=ovmf/ovmf-code-$(KARCH).fd,readonly=on \
	        -drive if=pflash,unit=1,format=raw,file=ovmf/ovmf-vars-$(KARCH).fd \
	        -cdrom $(IMAGE_NAME).iso \
	        -S -s \
	        $(QEMUFLAGS)

.PHONY: run-hdd-riscv64
run-hdd-riscv64: ovmf/ovmf-code-$(KARCH).fd ovmf/ovmf-vars-$(KARCH).fd $(IMAGE_NAME).hdd
	qemu-system-$(KARCH) \
		-M virt \
		-cpu rv64 \
		-device ramfb \
		-device qemu-xhci \
		-device usb-kbd \
		-device usb-mouse \
		-drive if=pflash,unit=0,format=raw,file=ovmf/ovmf-code-$(KARCH).fd,readonly=on \
		-drive if=pflash,unit=1,format=raw,file=ovmf/ovmf-vars-$(KARCH).fd \
		-hda $(IMAGE_NAME).hdd \
		$(QEMUFLAGS)

.PHONY: run-loongarch64
run-loongarch64: ovmf/ovmf-code-$(KARCH).fd ovmf/ovmf-vars-$(KARCH).fd $(IMAGE_NAME).iso
	qemu-system-$(KARCH) \
	        -M virt \
	        -cpu la464 \
		-device ramfb \
		-device qemu-xhci \
		-device usb-kbd \
		-device usb-mouse \
		-drive if=pflash,unit=0,format=raw,file=ovmf/ovmf-code-$(KARCH).fd,readonly=on \
		-drive if=pflash,unit=1,format=raw,file=ovmf/ovmf-vars-$(KARCH).fd \
	        -cdrom $(IMAGE_NAME).iso \
	        $(QEMUFLAGS)

.PHONY: run-debug-loongarch64
run-debug-loongarch64: ovmf/ovmf-code-$(KARCH).fd ovmf/ovmf-vars-$(KARCH).fd $(IMAGE_NAME).iso
	qemu-system-$(KARCH) \
	        -M virt \
	        -cpu la464 \
	        -device ramfb \
	        -device qemu-xhci \
	        -device usb-kbd \
	        -device usb-mouse \
	        -drive if=pflash,unit=0,format=raw,file=ovmf/ovmf-code-$(KARCH).fd,readonly=on \
	        -drive if=pflash,unit=1,format=raw,file=ovmf/ovmf-vars-$(KARCH).fd \
	        -cdrom $(IMAGE_NAME).iso \
	        -S -s \
	        $(QEMUFLAGS)

.PHONY: run-hdd-loongarch64
run-hdd-loongarch64: ovmf/ovmf-code-$(KARCH).fd ovmf/ovmf-vars-$(KARCH).fd $(IMAGE_NAME).hdd
	qemu-system-$(KARCH) \
		-M virt \
		-cpu la464 \
		-device ramfb \
		-device qemu-xhci \
		-device usb-kbd \
		-device usb-mouse \
		-drive if=pflash,unit=0,format=raw,file=ovmf/ovmf-code-$(KARCH).fd,readonly=on \
		-drive if=pflash,unit=1,format=raw,file=ovmf/ovmf-vars-$(KARCH).fd \
		-hda $(IMAGE_NAME).hdd \
		$(QEMUFLAGS)


.PHONY: run-bios
run-bios: $(IMAGE_NAME).iso
	qemu-system-$(KARCH) \
	        -M q35 \
	        -cdrom $(IMAGE_NAME).iso \
	        -boot d \
	        $(QEMUFLAGS)

.PHONY: run-debug-bios
run-debug-bios: $(IMAGE_NAME).iso
	qemu-system-$(KARCH) \
	        -M q35 \
	        -cdrom $(IMAGE_NAME).iso \
	        -boot d \
	        -S -s \
	        $(QEMUFLAGS)

.PHONY: run-hdd-bios
run-hdd-bios: $(IMAGE_NAME).hdd
	qemu-system-$(KARCH) \
		-M q35 \
		-hda $(IMAGE_NAME).hdd \
		$(QEMUFLAGS)
	
ovmf/ovmf-code-$(KARCH).fd:
	mkdir -p ovmf
	curl -Lo $@ https://github.com/osdev0/edk2-ovmf-nightly/releases/latest/download/ovmf-code-$(KARCH).fd
	case "$(KARCH)" in \
		aarch64) dd if=/dev/zero of=$@ bs=1 count=0 seek=67108864 2>/dev/null;; \
		loongarch64) dd if=/dev/zero of=$@ bs=1 count=0 seek=5242880 2>/dev/null;; \
		riscv64) dd if=/dev/zero of=$@ bs=1 count=0 seek=33554432 2>/dev/null;; \
	esac

ovmf/ovmf-vars-$(KARCH).fd:
	mkdir -p ovmf
	curl -Lo $@ https://github.com/osdev0/edk2-ovmf-nightly/releases/latest/download/ovmf-vars-$(KARCH).fd
	case "$(KARCH)" in \
		aarch64) dd if=/dev/zero of=$@ bs=1 count=0 seek=67108864 2>/dev/null;; \
		loongarch64) dd if=/dev/zero of=$@ bs=1 count=0 seek=5242880 2>/dev/null;; \
		riscv64) dd if=/dev/zero of=$@ bs=1 count=0 seek=33554432 2>/dev/null;; \
	esac

limine/limine:
	rm -rf limine
	git clone https://github.com/limine-bootloader/limine.git --branch=v9.x-binary --depth=1
	$(MAKE) -C limine

.PHONY: third_party
third_party:
	@if [ -f third_party/ascii/Cargo.toml ]; then \
		echo "third_party present"; \
		exit 0; \
	fi; \
	echo "third_party/ascii missing or empty — fetching..."; \
	rm -rf third_party/ascii; \
	mkdir -p third_party; \
	git clone https://github.com/tomprogrammer/rust-ascii third_party/ascii

.PHONY: userland
userland:
ifeq ($(MODE),native)
	cd compositor && RUSTFLAGS="-C link-arg=-T$(CURDIR)/compositor/link.ld -C relocation-model=static -C code-model=large -C target-cpu=x86-64" cargo build --release --target $(RUST_TARGET)
	cd apps/demo_app && RUSTFLAGS="-C link-arg=-T$(CURDIR)/userland/linker.ld -C relocation-model=static -C code-model=large -C target-cpu=x86-64" cargo build --release --target x86_64-unknown-none
	cd apps/task_list && RUSTFLAGS="-C link-arg=-T$(CURDIR)/userland/linker.ld -C relocation-model=static -C code-model=large -C target-cpu=x86-64" cargo build --release --target x86_64-unknown-none
	cd apps/graph_viewer && RUSTFLAGS="-C link-arg=-T$(CURDIR)/userland/linker.ld -C relocation-model=static -C code-model=large -C target-cpu=x86-64" cargo build --release --target x86_64-unknown-none
	cd apps/init && RUSTFLAGS="-C link-arg=-T$(CURDIR)/userland/linker.ld -C relocation-model=static -C code-model=large -C target-cpu=x86-64" cargo build --release --target x86_64-unknown-none
	cd drivers/keyboard_driver && RUSTFLAGS="-C link-arg=-T$(CURDIR)/userland/linker.ld -C relocation-model=static -C code-model=large -C target-cpu=x86-64" cargo build --release --target x86_64-unknown-none
	cd drivers/mouse_driver && RUSTFLAGS="-C link-arg=-T$(CURDIR)/userland/linker.ld -C relocation-model=static -C code-model=large -C target-cpu=x86-64" cargo build --release --target x86_64-unknown-none
	cd drivers/framebuffer_driver && RUSTFLAGS="-C link-arg=-T$(CURDIR)/userland/linker.ld -C relocation-model=static -C code-model=large -C target-cpu=x86-64" cargo build --release --target x86_64-unknown-none
else ifeq ($(MODE),hosted)
	cargo build -p compositor --bin compositor --features host --target x86_64-unknown-linux-gnu
	cargo build -p demo_app --bin demo_app --features host --target x86_64-unknown-linux-gnu
	cargo build -p task_list --bin task_list --features host --target x86_64-unknown-linux-gnu
	cargo build -p thing_host --bin thing_host --target x86_64-unknown-linux-gnu
endif

.PHONY: compositor
compositor:
ifeq ($(MODE),native)
	$(MAKE) all
	$(MAKE) run
else ifeq ($(MODE),hosted)
	$(MAKE) userland
	cargo run -p thing_host --bin thing_host --features "$(HOST_FEATURES)" --target x86_64-unknown-linux-gnu -- --launch-app target/x86_64-unknown-linux-gnu/debug/compositor
endif

.PHONY: demo_app
demo_app:
ifeq ($(MODE),native)
	$(MAKE) all
	$(MAKE) run
else ifeq ($(MODE),hosted)
	$(MAKE) userland
	cargo run -p thing_host --bin thing_host --features "$(HOST_FEATURES)" --target x86_64-unknown-linux-gnu -- --launch-app target/x86_64-unknown-linux-gnu/debug/demo_app
endif

.PHONY: run-hosted
run-hosted:
	docker compose -f docker-compose.neo4j.yml up -d
	GRAPH_BACKEND=neo4j \
	NEO4J_URI=$(HOST_NEO4J_URI) \
	NEO4J_USER=$(HOST_NEO4J_USER) \
	NEO4J_PASSWORD=$(HOST_NEO4J_PASSWORD) \
	cargo run -p thing_host --bin thing_host --features neo4j --target x86_64-unknown-linux-gnu


.PHONY: kernel
kernel: third_party
	$(MAKE) -C kernel

$(IMAGE_NAME).iso: limine/limine kernel userland
	rm -rf iso_root
	mkdir -p iso_root/boot
	cp -v clouds.bmp iso_root/
	cp -v kernel/kernel iso_root/boot/
	cp -v target/$(RUST_TARGET)/release/compositor iso_root/boot/
	cp -v target/$(RUST_TARGET)/release/init iso_root/boot/
	cp -v target/$(RUST_TARGET)/release/demo_app iso_root/boot/
	cp -v target/$(RUST_TARGET)/release/task_list iso_root/boot/
	cp -v target/$(RUST_TARGET)/release/graph_viewer iso_root/boot/
	cp -v target/$(RUST_TARGET)/release/keyboard_driver iso_root/boot/
	cp -v target/$(RUST_TARGET)/release/mouse_driver iso_root/boot/
	cp -v target/$(RUST_TARGET)/release/framebuffer_driver iso_root/boot/
	mkdir -p iso_root/boot/limine
	cp -v limine.conf iso_root/boot/limine/
	mkdir -p iso_root/EFI/BOOT
ifeq ($(KARCH),x86_64)
	cp -v limine/limine-bios.sys limine/limine-bios-cd.bin limine/limine-uefi-cd.bin iso_root/boot/limine/
	cp -v limine/BOOTX64.EFI iso_root/EFI/BOOT/
	cp -v limine/BOOTIA32.EFI iso_root/EFI/BOOT/
	xorriso -as mkisofs -b boot/limine/limine-bios-cd.bin \
		-no-emul-boot -boot-load-size 4 -boot-info-table \
		--efi-boot boot/limine/limine-uefi-cd.bin \
		-efi-boot-part --efi-boot-image --protective-msdos-label \
		iso_root -o $(IMAGE_NAME).iso
	./limine/limine bios-install $(IMAGE_NAME).iso
endif
ifeq ($(KARCH),aarch64)
	cp -v limine/limine-uefi-cd.bin iso_root/boot/limine/
	cp -v limine/BOOTAA64.EFI iso_root/EFI/BOOT/
	xorriso -as mkisofs \
		--efi-boot boot/limine/limine-uefi-cd.bin \
		-efi-boot-part --efi-boot-image --protective-msdos-label \
		iso_root -o $(IMAGE_NAME).iso
endif
ifeq ($(KARCH),riscv64)
	cp -v limine/limine-uefi-cd.bin iso_root/boot/limine/
	cp -v limine/BOOTRISCV64.EFI iso_root/EFI/BOOT/
	xorriso -as mkisofs \
		--efi-boot boot/limine/limine-uefi-cd.bin \
		-efi-boot-part --efi-boot-image --protective-msdos-label \
		iso_root -o $(IMAGE_NAME).iso
endif
ifeq ($(KARCH),loongarch64)
	cp -v limine/limine-uefi-cd.bin iso_root/boot/limine/
	cp -v limine/BOOTLOONGARCH64.EFI iso_root/EFI/BOOT/
	xorriso -as mkisofs \
		--efi-boot boot/limine/limine-uefi-cd.bin \
		-efi-boot-part --efi-boot-image --protective-msdos-label \
		iso_root -o $(IMAGE_NAME).iso
endif
	rm -rf iso_root

$(IMAGE_NAME).hdd: limine/limine kernel
	rm -f $(IMAGE_NAME).hdd
	dd if=/dev/zero bs=1M count=0 seek=64 of=$(IMAGE_NAME).hdd
	sgdisk $(IMAGE_NAME).hdd -n 1:2048 -t 1:ef00
ifeq ($(KARCH),x86_64)
	./limine/limine bios-install $(IMAGE_NAME).hdd
endif
	mformat -i $(IMAGE_NAME).hdd@@1M
	mmd -i $(IMAGE_NAME).hdd@@1M ::/EFI ::/EFI/BOOT ::/boot ::/boot/limine
	mcopy -i $(IMAGE_NAME).hdd@@1M kernel/bin-$(KARCH)/kernel ::/boot
	mcopy -i $(IMAGE_NAME).hdd@@1M limine.conf ::/boot/limine
ifeq ($(KARCH),x86_64)
	mcopy -i $(IMAGE_NAME).hdd@@1M limine/limine-bios.sys ::/boot/limine
	mcopy -i $(IMAGE_NAME).hdd@@1M limine/BOOTX64.EFI ::/EFI/BOOT
	mcopy -i $(IMAGE_NAME).hdd@@1M limine/BOOTIA32.EFI ::/EFI/BOOT
endif
ifeq ($(KARCH),aarch64)
	mcopy -i $(IMAGE_NAME).hdd@@1M limine/BOOTAA64.EFI ::/EFI/BOOT
endif
ifeq ($(KARCH),riscv64)
	mcopy -i $(IMAGE_NAME).hdd@@1M limine/BOOTRISCV64.EFI ::/EFI/BOOT
endif
ifeq ($(KARCH),loongarch64)
	mcopy -i $(IMAGE_NAME).hdd@@1M limine/BOOTLOONGARCH64.EFI ::/EFI/BOOT
endif

.PHONY: host-demo-neo4j
host-demo-neo4j:
	docker compose -f docker-compose.neo4j.yml up -d
	GRAPH_BACKEND=neo4j \
	NEO4J_URI=$(HOST_NEO4J_URI) \
	NEO4J_USER=$(HOST_NEO4J_USER) \
	NEO4J_PASSWORD=$(HOST_NEO4J_PASSWORD) \
	MODE=hosted HOST_FEATURES=neo4j $(MAKE) demo_app

.PHONY: clean
clean:
	$(MAKE) -C kernel clean
	rm -rf iso_root $(IMAGE_NAME).iso $(IMAGE_NAME).hdd
	# Remove downloaded or generated tooling and rust build artifacts
	rm -rf limine ovmf
	rm -rf target
	rm -rf compositor/target
	rm -rf userland/target
	rm -rf apps/*/target
	rm -rf drivers/*/target
	rm -rf drivers/*/target/*
	rm -rf */target
	rm -rf build

.PHONY: distclean
distclean: clean
	$(MAKE) -C kernel distclean
	rm -rf limine ovmf target
