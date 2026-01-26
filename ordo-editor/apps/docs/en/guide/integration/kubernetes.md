# Kubernetes Integration

<img src="/integration/kubernetes.png" alt="Kubernetes Logo" width="120" style="margin-bottom: 20px;" />

Ordo can be easily deployed into a [Kubernetes](https://kubernetes.io/) cluster. As a stateless service, Ordo works perfectly in a K8s environment.

## Deployment Manifests

Below is a standard Kubernetes manifest including a Deployment and a Service.

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

## Configuration Notes

- **Health Checks**: Liveness and Readiness probes are configured pointing to the `/health` endpoint.
- **Ports**: Container exposes 8080 (HTTP) and 50051 (gRPC).
- **Resource Limits**: Adjust CPU and memory requests/limits based on your load.
- **Scaling**: Easily scale the service by changing the `replicas` count.

## Deployment

Save the above configuration as `ordo-k8s.yaml` and run:

```bash
kubectl apply -f ordo-k8s.yaml
```

Check deployment status:

```bash
kubectl get pods -l app=ordo
```
