# External build wrapper; kernel-side rules live in Kbuild.
KVER ?= $(shell uname -r)
KDIR ?= /lib/modules/$(KVER)/build
BINDGEN ?= $(shell command -v bindgen 2>/dev/null || echo /usr/local/lib/predator-sense/bindgen)
.PHONY: all clean
all:
	@test -f "$(KDIR)/rust/libkernel.rmeta" || { echo "CONFIG_RUST=y and prepared Rust metadata are required"; exit 1; }
	BINDGEN="$(BINDGEN)" bash ./generate-bindings.sh "$(KDIR)" "$(CURDIR)/bindings_generated.rs"
	python3 ./abi_check.py "$(KDIR)" "$(CURDIR)/bindings_generated.rs"
	$(MAKE) -C "$(KDIR)" M="$(CURDIR)" modules
clean:
	$(MAKE) -C "$(KDIR)" M="$(CURDIR)" clean
