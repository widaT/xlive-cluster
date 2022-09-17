# xlive register

xlive-register是一个实现更新的映射表，用于源站和第一层缓存（xlive-cache）也可能是边缘节点(xilve-edge），房间号（app_name）查找到推流ip.
同时xlive-register自动收集推所有流服务器ip，还可以做存活判断。

对外提供两个http接口

- /servers_info

获取所有有负载的xlive-origin

- /channels_info

获取所有存活的推流app_name