# Kubernetes 集成

<img src="/integration/kubernetes.png" alt="Kubernetes Logo" width="120" style="margin-bottom: 20px;" />

Ordo 可以轻松部署到 [Kubernetes](https://kubernetes.io/) 集群中。作为一个无状态服务（Stateless Service），Ordo 非常适合在 K8s 环境中运行。

## 部署清单

以下是一个标准的 Kubernetes 部署清单，包含 Deployment 和 Service。

### Deployment

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: ordo-server
  labels:
    app: ordo
spec:
  replicas: 2
  selector:
    matchLabels:
      app: ordo
  template:
    metadata:
      labels:
        app: ordo
    spec:
      containers:
        - name: ordo-server
          image: ghcr.io/pama-lee/ordo-server:latest
          imagePullPolicy: Always
          ports:
            - containerPort: 8080
              name: http
            - containerPort: 50051
              name: grpc
          livenessProbe:
            httpGet:
              path: /health
              port: http
            initialDelaySeconds: 5
            periodSeconds: 10
          readinessProbe:
            httpGet:
              path: /health
              port: http
            initialDelaySeconds: 2
            periodSeconds: 5
          resources:
            requests:
              cpu: '100m'
              memory: '128Mi'
            limits:
              cpu: '500m'
              memory: '512Mi'
          env:
            - name: RUST_LOG
              value: 'info'
```

### Service

```yaml
apiVersion: v1
kind: Service
metadata:
  name: ordo-service
spec:
  selector:
    app: ordo
  ports:
    - protocol: TCP
      port: 80
      targetPort: 8080
      name: http
    - protocol: TCP
      port: 50051
      targetPort: 50051
      name: grpc
  type: ClusterIP
```

## 配置说明

- **健康检查**：配置了 Liveness 和 Readiness探针，均指向 `/health` 端点。
- **端口**：容器暴露 8080 (HTTP) 和 50051 (gRPC)。
- **资源限制**：建议根据实际负载调整 CPU 和内存限制。
- **水平扩展**：可以通过修改 `replicas` 数量轻松扩展服务实例。

## 部署命令

将上述配置保存为 `ordo-k8s.yaml`，然后执行：

```bash
kubectl apply -f ordo-k8s.yaml
```

检查部署状态：

```bash
kubectl get pods -l app=ordo
```
