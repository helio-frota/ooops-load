import fs from 'node:fs';
import path from 'node:path';
import axios from 'axios';
import pLimit from 'p-limit';

const args = process.argv.slice(2);

function showHelp() {
  console.log(`
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
  npm start -- \\
    --e=http://localhost:8080/api/v2/sbom \\
    --s=/home/user/Downloads/atlas-s3/sbom/spdx/ \\
    --c=10 \\
    --b=700 \\
`);
}

if (args.includes('--h')) {
  showHelp();
  process.exit(0);
}

function getArg(name: string, defaultValue?: string): string {
  const prefix = `--${name}=`;
  const found = args.find(arg => arg.startsWith(prefix));
  if (found) {
    return found.slice(prefix.length);
  }
  if (!defaultValue) {
    throw new Error(`Missinng required argument: --${name}`);
  }
  return defaultValue;
}

const URL = getArg('e');
const SOURCE_DIR = getArg('s');
const CONCURRENCY = parseInt(getArg('c', '4'), 10);
const BATCH_SIZE = parseInt(getArg('b', '200'), 10);
const LABEL = 'aaa';
const ERROR_LOG = 'errors.log';

const limit = pLimit(CONCURRENCY);

async function uploadFile(filePath: string): Promise<void> {
  try {
    const data = fs.createReadStream(filePath);

    const resp = await axios.post(`${URL}?labels=${LABEL}`, data, {
      headers: { 'Content-Type': 'application/json' },
      maxContentLength: Infinity,
      maxBodyLength: Infinity,
      timeout: 30000,
    });
    console.log(`${resp.status} -> ${filePath}`);
  } catch (err: any) {
    console.error(`Failed ${filePath}: ${err.message}`);
    fs.appendFileSync(ERROR_LOG, filePath + '\n');
  }
}

async function processBatch(files: string[]): Promise<void> {
  const tasks = files.map(filePath => limit(() => uploadFile(filePath)));
  await Promise.all(tasks);
}

async function main(): Promise<void> {
  const dir = await fs.promises.opendir(SOURCE_DIR);
  let batch: string[] = [];

  for await (const dirent of dir) {
    if (dirent.isFile()) {
      batch.push(path.join(SOURCE_DIR, dirent.name));
      if (batch.length >= BATCH_SIZE) {
        await processBatch(batch);
        batch = [];
      }
    }
  }

  if (batch.length > 0) {
    await processBatch(batch);
  }

  console.log('Done!');
  console.log(`Failures: ${ERROR_LOG}`);
}

main().catch(console.error);
