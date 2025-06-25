FROM --platform=linux/arm64 public.ecr.aws/amazonlinux/amazonlinux:2023@sha256:39ba4d54c9805f781646dea291e93a566d677a430ee77dc9e2fd733b0bf4e40a

ARG NITRO_CLI_VER=1.4.2-0.amzn2023
ARG SOURCE_DATE_EPOCH
ENV SOURCE_DATE_EPOCH=${SOURCE_DATE_EPOCH}

# Install pinned nitro-cli and lock it to ensure reproducibility
RUN --mount=type=cache,target=/var/cache/dnf \
    dnf install -y 'dnf-command(versionlock)' && \
    dnf install -y aws-nitro-enclaves-cli-${NITRO_CLI_VER}.aarch64 \
                   aws-nitro-enclaves-cli-devel-${NITRO_CLI_VER}.aarch64 && \
    dnf versionlock add aws-nitro-enclaves-cli-${NITRO_CLI_VER}.aarch64 \
                        aws-nitro-enclaves-cli-devel-${NITRO_CLI_VER}.aarch64 && \
    rpm -V aws-nitro-enclaves-cli && \
    dnf clean all

ENTRYPOINT ["nitro-cli"] 