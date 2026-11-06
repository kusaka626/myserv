FROM myserv-native-check
RUN --mount=type=secret,id=proxy_ca,required=true \
    apt-get -o Acquire::https::CaInfo=/run/secrets/proxy_ca update && \
    apt-get -o Acquire::https::CaInfo=/run/secrets/proxy_ca install -y --no-install-recommends openssh-server sudo && \
    rm -rf /var/lib/apt/lists/*
RUN useradd -m -s /bin/bash ubuntu && useradd -m -s /bin/bash limited && \
    usermod -aG sudo ubuntu && mkdir -p /run/sshd && \
    printf 'PermitRootLogin yes\nPasswordAuthentication yes\nUsePAM no\n' > /etc/ssh/sshd_config.d/myserv-test.conf
# Only disposable test credentials, supplied by the local fixture runner.
CMD ["/usr/sbin/sshd", "-D", "-e"]
