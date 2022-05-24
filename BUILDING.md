# Building

If you would like to compile this program yourself, you can follow these instructions.

The [build.yml](.github/workflows/build.yml) shows an automated build process.

## Prerequisites

First, install Cargo and the Rust compiler using [rustup](https://rustup.rs/)

If you are on Windows, install and setup [vcpkg](https://vcpkg.io/en/getting-started.html)

### NVIDIA CUDA

If you would like to build Squad Mortar Helper with NVIDIA CUDA support, install the [NVIDIA CUDA Toolkit](https://developer.nvidia.com/cuda-toolkit)

To disable NVIDIA CUDA support, add `--no-default-features` onto the end of the `cargo build` commands below.

## Windows

```batch
vcpkg install leptonica:x64-windows-static-md
cargo build --release --features gpu-ptx-vendored
```

## Linux

```bash
sudo apt update
sudo apt install build-essential autoconf automake libtool pkg-config libpng-dev libjpeg8-dev libtiff5-dev zlib1g-dev libleptonica-dev libxcb-randr0-dev libxcb-shm0-dev libxcb-shape0-dev libxcb-xfixes0-dev libclang-dev -y
git clone https://github.com/tesseract-ocr/tesseract.git --depth 1 --single-branch --branch 5.1.0
export CFLAGS=-fPIC
export CXXFLAGS=-fPIC
cd tesseract
./autogen.sh
./configure --disable-graphics --disable-legacy --disable-doc --disable-openmp --without-curl --without-archive --without-tensorflow --enable-shared=no --enable-static=yes
make
mkdir -p ../lib/tesseract/linux-x64
cp .libs/libtesseract.a ../lib/tesseract/linux-x64/libtesseract.a
cd ../
cargo build --release --features gpu-ptx-vendored
```
