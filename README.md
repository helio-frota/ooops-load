# ooops-load

Specific use case uploader

```
 npm start -- --h

> ooops-load@0.1.0 start
> ts-node src/index.ts --h

Usage:
  npm start -- --e=<ENDPOINT> --s=<DIR> [options]

Required:
  --e=<ENDPOINT>    Endpoint (http://localhost:8080/api/v2/sbom or http://localhost:8080/api/v2/advisory)
  --s=<DIR>    Source directory containing the JSON files (e.g. /home/foobar/myjsonfilesdir/)

Options:
  --c=<N>      Concurrent uploads (default: 4)
  --b=<N>      Batch size per round (default: 200)
  --h          Help

Example:
  npm start -- \
    --e=http://localhost:8080/api/v2/sbom \
    --s=/home/user/Downloads/atlas-s3/sbom/spdx/ \
    --c=10 \
    --b=700 \
```

