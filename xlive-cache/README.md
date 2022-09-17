# xlive cache

xlive 缓存节点，xilve的缓存节点支持多层级。

## 一级缓存

一级缓存比较特别，一级缓存上游就是源站，作为保护源站防止被拉流端拉爆。一级缓存通常会合源站一个内网，一级缓存通过xilve-register来发现推流源机器，使用xilve-register可以让 xlive-origin和xilve-cache一级缓存实现无状态，可以在k8s中横向拓展。
