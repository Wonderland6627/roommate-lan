FROM python:3.12-slim

WORKDIR /app

# Faster/more reliable on mainland China hosts (Tencent Lighthouse, etc.).
RUN sed -i 's/deb.debian.org/mirrors.aliyun.com/g' /etc/apt/sources.list.d/debian.sources \
  && sed -i 's/security.debian.org/mirrors.aliyun.com/g' /etc/apt/sources.list.d/debian.sources \
  && apt-get update \
  && apt-get install -y --no-install-recommends tzdata \
  && ln -snf /usr/share/zoneinfo/Asia/Shanghai /etc/localtime \
  && echo Asia/Shanghai > /etc/timezone \
  && rm -rf /var/lib/apt/lists/*

COPY requirements.txt .
# PyPI often fails with "versions: none" from CN; use Tsinghua mirror.
RUN pip install --no-cache-dir \
  -i https://pypi.tuna.tsinghua.edu.cn/simple \
  --trusted-host pypi.tuna.tsinghua.edu.cn \
  -r requirements.txt

COPY app ./app

ENV DATA_DIR=/data
# Midnight log rotation + localtime; asctime / traffic stamps also forced to UTC+8 in code.
ENV TZ=Asia/Shanghai
RUN mkdir -p /data

EXPOSE 8081
CMD ["uvicorn", "app.main:app", "--host", "0.0.0.0", "--port", "8081"]
