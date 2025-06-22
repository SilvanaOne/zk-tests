;; manifest.scm  –– Guix packages that replace every “core-*” image
(use-modules (gnu packages)       ; meta-import, gives us 'specifications->manifest'
             (gnu packages base)  ; bash-minimal, busybox …
             (gnu packages gcc)   ; gcc, binutils
             (gnu packages llvm)  ; clang, lld, compiler-rt
             (gnu packages rust)
             (gnu packages tls)   ; openssl
             (gnu packages compression) ; zlib, zstd
             (gnu packages linux) ; libseccomp, libunwind, musl
             (gnu packages version-control) ; git
             (gnu packages pkgconfig)
             (gnu packages admin) ; ca-certificates
             (gnu packages cpio)
             (gnu packages networking) ; socat
             (gnu packages jq))

(specifications->manifest
  (list
    ;; shells & basic userland
    "bash-minimal" "busybox"

    ;; tool-chain / build essentials
    "gcc-toolchain" "binutils" "clang-toolchain" "llvm"
    "rust"                        ; pick a pin, e.g. "rust@1.78"
    "pkg-config"

    ;; crypto / compression
    "openssl" "zlib" "zstd"

    ;; runtime libs
    "musl" "libunwind" "libffi" "libseccomp"

    ;; helpers
    "git" "jq" "socat" "cpio" "ca-certificates"))