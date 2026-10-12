# 知舟服务台：MinerU（pipeline 后端，CPU 可用）本地构建镜像
FROM python:3.12-slim
ARG PIP_INDEX=https://pypi.org/simple
RUN apt-get update && apt-get install -y --no-install-recommends \
      fonts-noto-core fonts-noto-cjk libgl1 libglib2.0-0 libgomp1 tini curl \
    && rm -rf /var/lib/apt/lists/*
RUN pip install --no-cache-dir -i ${PIP_INDEX} "mineru[core]>=2.5"
ENV HF_HOME=/data/huggingface MODELSCOPE_CACHE=/data/modelscope MINERU_MODEL_SOURCE=modelscope
EXPOSE 8000
ENTRYPOINT ["tini", "--"]
CMD ["mineru-api", "--host", "0.0.0.0", "--port", "8000"]
