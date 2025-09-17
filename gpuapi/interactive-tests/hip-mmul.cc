// -*- c-basic-offset: 2; indent-tabs-mode: nil -*-

// Copyright 2025 Norwegian AI Cloud.
// SPDX-License-Identifier: MIT

// This performs a tiled mmul in HIP. (Culled from an old test case, itself culled from CUDA code.)
//
// To compile this, load the HIP tool chain if you need to, then `make hip-mmul`.
//
// Run it without arguments or with some options:
//
//  -d<n>  The device, default 0
//  -i<n>  The number of iterations, default 1
//
// A single iteration takes about 12s on an AMD Vega 10 XL/XT.

#include <cstdio>
#include <cstdlib>
#include <limits>
#include <algorithm>
#include <iostream>
#include <hip/hip_runtime.h>

static void* malloc_device(size_t nbytes) {
  void* p;
  hipError_t err;
  if ((err = hipMalloc(&p, nbytes)) != 0) {
    fprintf(stderr, "malloc_device %zu bytes failed: %d\n", nbytes, err);
    abort();
  }
  return p;
}

static void copy_to_device(void* dev_dest, void* host_src, size_t nbytes) {
  hipError_t err;
  if ((err = hipMemcpy(dev_dest, host_src, nbytes, hipMemcpyHostToDevice)) != 0) {
    fprintf(stderr, "copy_to_device %zu bytes failed: %d\n", nbytes, err);
    abort();
  }
}

static void device_synchronize() {
  hipError_t err;
  if ((err = hipDeviceSynchronize()) != 0) {
    fprintf(stderr, "device_synchronize failed: %d\n", err);
    abort();
  }
}

template<size_t N, size_t M, typename T>
void init_matrix(T mat[N][M], int scheme) {
  T v = T(1);
  for ( size_t i=0 ; i < N ; i++ ) {
    for ( size_t j=0 ; j < M ; j++ ) {
      mat[i][j] = v;
      v = v + (scheme + 1);
      if (v > 31) {
	v -= 31;
      }
    }
  }
}

constexpr size_t N = 20000;	// Dimension size
constexpr size_t S = 16;	// Tile dimension size (reasonable guess?)
constexpr size_t NUMTILES = (N + (S - 1)) / S;

// A, B and C are NxN float arrays in row-major order, densely packed

// The way this works is that each kernel application computes the value of one specific result
// element.  This element is at (row,col) and is the usual dot product of the A row and B column it
// is in.  But the trick is that the element is computed by collectively loading tiles of A and B
// into shared thread block memory, then all threads in the block compute partial results using the
// collectively loaded inputs, then they load new tile inputs, and so on, for the entire tile row
// and tile column that the output tile is in.
//
// Effectively, the threads collectively populate a cache and then hit that repeatedly.
//
// The intuition for the algorithm is most clearly seen in ../mmul/mmul.cc:v3 but with the variation
// that the kk loop is moved inside the ii,jj loops, see comments there.  A result tile is selected
// and computed from the source tiles in the A row and B column that intersect at it.

__global__ void mmul_kernel(float* A, float* B, float* C) {
  __shared__ float tile_A[S][S];
  __shared__ float tile_B[S][S];

  // These are global coordinates in C of the element being computed.
  int row = blockIdx.y * blockDim.y + threadIdx.y;
  int col = blockIdx.x * blockDim.x + threadIdx.x;

  // These are local coordinates within the SxS input tiles.
  int tile_x = threadIdx.x;
  int tile_y = threadIdx.y;

  float acc = 0.0f;
  for ( int tile=0 ; tile < NUMTILES; tile++ ) {
    // col_A and row_B are global coordinates for input values being read into the shared tiles.
    int col_A = tile*S + tile_x;
    if (row < N && col_A < N) {
      tile_A[tile_y][tile_x] = A[row*N + col_A];
    } else {
      tile_A[tile_y][tile_x] = 0.0f;
    }
    int row_B = (tile*S + tile_y);
    if (row_B < N && col < N) {
      tile_B[tile_y][tile_x] = B[row_B*N + col];
    } else {
      tile_B[tile_y][tile_x] = 0.0f;
    }

    __syncthreads();

    for ( int k=0; k < S; k++ ) {
      acc += tile_A[tile_y][k] * tile_B[k][tile_x];
    }

    __syncthreads();
  }

  if (row < N && col < N) {
    C[row*N + col] = acc;
  }
}

int main(int argc, char** argv) {
  int device = 0;
  int iterations = 1;

  for ( int k=1 ; k < argc ; k++ ) {
    sscanf(argv[k], "-d%d", &device);
    sscanf(argv[k], "-i%d", &iterations);
  }

  // Allocate dynamically to avoid linker errors.
  auto A = new float[N][N];
  auto B = new float[N][N];

  init_matrix<N,N>(A, /* scheme= */ 0);
  init_matrix<N,N>(B, /* scheme= */ 1);

  if (hipSetDevice(device) != 0) {
    abort();
  }

  size_t asize = N*N*sizeof(float);

  float* dev_A = (float*)malloc_device(asize);
  copy_to_device(dev_A, A, asize);
  float* dev_B = (float*)malloc_device(asize);
  copy_to_device(dev_B, B, asize);
  float* dev_C = (float*)malloc_device(asize);
  // No need to clear C, whatever's there is overwritten.

  time_t then = time(NULL);;
  for ( int it=0 ; it < iterations ; it++ ) {
    dim3 threadsPerBlock(S, S);
    dim3 blocksPerGrid(NUMTILES, NUMTILES);
    mmul_kernel<<<blocksPerGrid, threadsPerBlock>>>(dev_A, dev_B, dev_C);
    device_synchronize();		// Or the timing is all wrong
  }
  time_t now = time(NULL);
  printf("Elapsed %d seconds\n", (int)(now - then));
}

