#include <cstddef>              // size_t
#include <iostream>

constexpr size_t NB = 250;	// Number of tiles in each dimension
constexpr size_t S = 10;	// Tile size
constexpr size_t N = NB*S;	// Dimension size

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

void mmul(float A[N][N], float B[N][N], float C[N][N]) {
  for ( size_t kk=0 ; kk < N ; kk+=S ) {
    for ( size_t jj=0 ; jj < N ; jj+=S ) {
      for ( size_t i=0 ; i < N ; i++ ) {
	for ( size_t j=jj ; j < jj + S ; j++ ) {
	  float sum = C[i][j];
	  for ( size_t k=kk ; k < kk + S ; k++ ) {
	    sum += A[i][k] * B[k][j];
	  }
	  C[i][j] = sum;
	}
      }
    }
  }
}

int main(int argc, char** argv) {
  // Avoid stack allocation for large N
  static float A[N][N], B[N][N], C[N][N];

  init_matrix<N,N>(A, /* scheme= */ 0);
  init_matrix<N,N>(B, /* scheme= */ 1);

  mmul(A, B, C);

  // Use the result
  float sum = 0.0f;
  for ( size_t j=0 ; j < N ; j++ ) {
      for ( size_t i=0 ; i < N ; i++ ) {
          sum += C[j][i];
      }
  }
  std::cout << sum << "\n";
}
