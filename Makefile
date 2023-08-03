MAYBE_MODEM := $(word 1, $(wildcard /dev/tty.usbmodem*))
MODEM := $(if $(MAYBE_MODEM),$(MAYBE_MODEM),$(error "modem not found"))
BAUD := 19200
PART := m328pb
PROGRAMMER := avrisp

AVRDUDE := avrdude -c $(PROGRAMMER) -p $(PART) -P $(MODEM) -b $(BAUD)

DEBUG_SPI ?= 0
ifeq ("$(DEBUG_SPI)","0")
FEATURES :=
else
FEATURES := --features=debug_spi
endif

.PHONY: all
all: hex text

.PHONY: hex
hex: firmware.hex

.PHONY: text
text: firmware.txt

.PHONY: flash
flash: firmware.hex
	$(AVRDUDE) -U flash:w:$^

# program fuse bytes
#   low fuse: all defaults + CKDIV8 (set frequency to 8MHz from internal oscillator)
.PHONY: fuse
fuse:
	$(AVRDUDE) -U lfuse:w:0xE2:m

.PHONY: read
read:
	$(AVRDUDE) -U flash:r:/dev/null:h

.PHONY: clean
clean:
	rm -f firmware.hex
	rm -f firmware.txt
	cargo clean

ELF := target/atmega328p/release/firmware.elf

firmware.hex: $(ELF)
	avr-objcopy -j .text -j .data -O ihex $^ $@

firmware.txt: $(ELF)
	avr-objdump -Sd $^ > $@

$(ELF): */src/*.rs Cargo.toml */Cargo.toml
	cargo build --release $(FEATURES)
