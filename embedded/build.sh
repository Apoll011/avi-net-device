#!/bin/bash
# Build script for AVI Embedded C API

set -e  # Exit on error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Default target
TARGET=${1:-xtensa-esp32-espidf}

echo -e "${GREEN}=== AVI Embedded Build Script ===${NC}"
echo ""

# Check if target is valid
case $TARGET in
    xtensa-esp32-espidf|xtensa-esp32s2-espidf|xtensa-esp32s3-espidf|riscv32imc-esp-espidf|riscv32imac-esp-espidf)
        echo -e "${GREEN}Building for target: $TARGET${NC}"
        ;;
    *)
        echo -e "${RED}Error: Invalid target '$TARGET'${NC}"
        echo ""
        echo "Valid targets:"
        echo "  - xtensa-esp32-espidf      (ESP32)"
        echo "  - xtensa-esp32s2-espidf    (ESP32-S2)"
        echo "  - xtensa-esp32s3-espidf    (ESP32-S3)"
        echo "  - riscv32imc-esp-espidf    (ESP32-C3)"
        echo "  - riscv32imac-esp-espidf   (ESP32-C6)"
        echo ""
        echo "Usage: $0 [target]"
        exit 1
        ;;
esac

# Check if cbindgen is installed
if ! command -v cbindgen &> /dev/null; then
    echo -e "${YELLOW}Warning: cbindgen not found. Header will not be regenerated.${NC}"
    echo -e "${YELLOW}Install with: cargo install cbindgen${NC}"
    SKIP_CBINDGEN=1
fi

# Step 1: Check if target is installed
echo ""
echo -e "${GREEN}[1/4] Checking Rust target...${NC}"
if ! rustup target list --installed | grep -q "$TARGET"; then
    echo -e "${YELLOW}Target not installed. Installing...${NC}"
    rustup target add "$TARGET"
else
    echo "Target already installed ✓"
fi

# Step 2: Build the Rust library
echo ""
echo -e "${GREEN}[2/4] Building Rust library...${NC}"
cargo build --release --target "$TARGET" --features c-api

if [ $? -eq 0 ]; then
    echo -e "${GREEN}Build successful ✓${NC}"
else
    echo -e "${RED}Build failed!${NC}"
    exit 1
fi

# Step 3: Generate header with cbindgen
echo ""
echo -e "${GREEN}[3/4] Generating C header...${NC}"
if [ -z "$SKIP_CBINDGEN" ]; then
    cbindgen --config cbindgen.toml --crate avi-embedded --output avi_embedded.h
    echo -e "${GREEN}Header generated ✓${NC}"
else
    echo -e "${YELLOW}Skipped (cbindgen not installed)${NC}"
fi

# Step 4: Copy artifacts to output directory
echo ""
echo -e "${GREEN}[4/4] Copying artifacts...${NC}"
mkdir -p output
cp "target/$TARGET/release/libavi_embedded.a" output/
if [ -f "avi_embedded.h" ]; then
    cp avi_embedded.h output/
fi

echo -e "${GREEN}Artifacts copied to output/ ✓${NC}"

# Summary
echo ""
echo -e "${GREEN}=== Build Complete ===${NC}"
echo ""
echo "Artifacts:"
echo "  📦 output/libavi_embedded.a"
if [ -f "output/avi_embedded.h" ]; then
    echo "  📄 output/avi_embedded.h"
fi
echo ""
echo "Next steps:"
echo "  1. Copy output/ contents to your ESP-IDF component"
echo "  2. Update your CMakeLists.txt"
echo "  3. Build your ESP-IDF project with 'idf.py build'"
echo ""

# Show file sizes
LIB_SIZE=$(du -h "output/libavi_embedded.a" | cut -f1)
echo -e "${GREEN}Library size: $LIB_SIZE${NC}"
