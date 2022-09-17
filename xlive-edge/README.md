# xlive-edge



## rtmp publisher

```bash
$ ffmpeg -re -i ~/Videos/dde-introduction.mp4 -c copy -f flv rtmp://localhost:1935/live
```

## http-flv

```bash
$ ffplay http://localhost:3000/live.flv
```

## rtmp

```bash
$ ffplay rtmp://localhost:1935/live
````