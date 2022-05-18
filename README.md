<!-- TOC -->

- [Visor](#visor)
  - [Rules](#rules)
  - [Author](#author)

<!-- /TOC -->

# Visor

系统资源监控

## Rules

- 监控服务器 CPU 和内存的使用情况，如果 CPU 或内存的使用情况超出指定范围，则停止运行时间最长的一个容器，同时将该容器的相关信息发送到企业微信群，让容器使用者可以重新启用该容器。
- 清理磁盘
  - 未使用的镜像
  - 未使用的数据卷
  - 退出超过指定天数的容器
  - /data/release 下超过指定天数的部署包
  - /data/ones/pkg 下超过指定天数的部署目录

## Author

K8sCat <rustpanic@gmail.com>
