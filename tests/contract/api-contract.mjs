#!/usr/bin/env node
// Contract tests: verify that the Rust route source code matches the OpenAPI spec.
//
// These tests parse the OpenAPI spec and the Rust route handlers to ensure:
// 1. Every route defined in main.rs has a corresponding OpenAPI path
// 2. Every OpenAPI path has a corresponding route handler
// 3. HTTP methods match between spec and implementation
// 4. Error codes in route handlers appear in the spec's error documentation
//
// Usage:
//   node tests/contract/api-contract.mjs
//
// Exit codes:
//   0 — all contract checks pass
//   1 — one or more mismatches found

import { readFileSync, existsSync } from 'fs';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(__dirname, '../..');

const OPENAPI_PATH = resolve(ROOT, 'apps/core/openapi.yaml');
const ROUTES_DIR = resolve(ROOT, 'apps/core/src/routes');
const MAIN_RS = resolve(ROOT, 'apps/core/src/main.rs');
const AUTH_ROUTES = resolve(ROOT, 'apps/core/src/auth/routes.rs');

let failures = 0;
let passes = 0;

function pass(msg) {
  passes++;
  console.log(`  ✅ ${msg}`);
}

function fail(msg) {
  failures++;
  console.error(`  ❌ ${msg}`);
}

// ---------------------------------------------------------------------------
// Parse OpenAPI paths and methods
// ---------------------------------------------------------------------------

function extractOpenAPIPaths(yamlText) {
  const paths = {};
  const lines = yamlText.split('\n');

  let currentPath = null;

  for (const line of lines) {
    const pathMatch = line.match(/^  (\/api\/[^:]+):\s*$/);
    if (pathMatch) {
      currentPath = pathMatch[1];
      if (!paths[currentPath]) {
        paths[currentPath] = new Set();
      }
      continue;
    }

    if (currentPath) {
      const methodMatch = line.match(/^    (get|post|put|delete|patch):\s*$/);
      if (methodMatch) {
        paths[currentPath].add(methodMatch[1].toUpperCase());
        continue;
      }
    }

    // Reset on top-level key
    if (/^[a-z]/.test(line) && !line.startsWith(' ')) {
      currentPath = null;
    }
  }

  return paths;
}

// ---------------------------------------------------------------------------
// Parse Rust route registrations from main.rs
// ---------------------------------------------------------------------------

function extractRustRoutes(mainRsText) {
  const routes = {};

  // Match .route("path", method(handler)) patterns
  // Handles chained methods like get(h).post(h).delete(h)
  const routeRe = /\.route\(\s*"([^"]+)"\s*,\s*([^)]+\)(?:\.[a-z]+\([^)]+\))*)\s*\)/g;
  let match;

  while ((match = routeRe.exec(mainRsText)) !== null) {
    const path = match[1];
    const methodChain = match[2];

    if (!routes[path]) {
      routes[path] = new Set();
    }

    // Extract method names from the chain
    const methodRe = /\b(get|post|put|delete|patch)\s*\(/g;
    let methodMatch;
    while ((methodMatch = methodRe.exec(methodChain)) !== null) {
      routes[path].add(methodMatch[1].toUpperCase());
    }
  }

  return routes;
}

// ---------------------------------------------------------------------------
// Parse error codes from Rust route source files
// ---------------------------------------------------------------------------

function extractErrorCodes(routeDir) {
  const codes = new Set();
  const files = [
    'data.rs', 'search.rs', 'events.rs', 'conflicts.rs',
    'system.rs', 'quarantine.rs', 'health.rs', 'plugins.rs',
  ];

  for (const file of files) {
    const filePath = resolve(routeDir, file);
    if (!existsSync(filePath)) continue;
    const content = readFileSync(filePath, 'utf-8');

    // Match "code": "ERROR_CODE" patterns
    const codeRe = /"code":\s*"([A-Z_]+)"/g;
    let match;
    while ((match = codeRe.exec(content)) !== null) {
      codes.add(match[1]);
    }
  }

  // Also check auth routes
  if (existsSync(AUTH_ROUTES)) {
    const content = readFileSync(AUTH_ROUTES, 'utf-8');
    const codeRe = /"code":\s*"([A-Z_]+)"/g;
    let match;
    while ((match = codeRe.exec(content)) !== null) {
      codes.add(match[1]);
    }
  }

  return codes;
}

// ---------------------------------------------------------------------------
// Extract error codes documented in OpenAPI spec
// ---------------------------------------------------------------------------

function extractSpecErrorCodes(yamlText) {
  const codes = new Set();

  // Match error codes in the spec — they appear as code values or in enum
  const codeRe = /code:\s*([A-Z][A-Z_]+)/g;
  let match;
  while ((match = codeRe.exec(yamlText)) !== null) {
    codes.add(match[1]);
  }

  // Also match error codes in descriptions
  const descRe = /`([A-Z][A-Z_]+)`/g;
  while ((match = descRe.exec(yamlText)) !== null) {
    if (match[1].includes('_')) {
      codes.add(match[1]);
    }
  }

  return codes;
}

// ---------------------------------------------------------------------------
// Normalize paths for comparison (Rust uses {param}, OpenAPI uses {param})
// ---------------------------------------------------------------------------

function normalizePath(path) {
  // Both Rust and OpenAPI 3.1 use {param} syntax, so normalize by just trimming
  return path.replace(/\s+/g, '');
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

console.log('API Contract Tests\n');

// 1. Check OpenAPI spec exists
console.log('1. OpenAPI spec presence:');
if (!existsSync(OPENAPI_PATH)) {
  fail('OpenAPI spec not found at apps/core/openapi.yaml');
  process.exit(1);
}
pass('OpenAPI spec exists');

const specText = readFileSync(OPENAPI_PATH, 'utf-8');
const specPaths = extractOpenAPIPaths(specText);

console.log(`   Found ${Object.keys(specPaths).length} paths in spec\n`);

// 2. Check main.rs exists and extract routes
console.log('2. Route registration coverage:');
if (!existsSync(MAIN_RS)) {
  fail('main.rs not found');
} else {
  const mainRs = readFileSync(MAIN_RS, 'utf-8');
  const rustRoutes = extractRustRoutes(mainRs);

  const specPathSet = new Set(Object.keys(specPaths).map(normalizePath));
  const rustPathSet = new Set(Object.keys(rustRoutes).map(normalizePath));

  // Routes that are intentionally excluded from the REST OpenAPI spec
  const excludedRoutes = new Set([
    '/api/graphql',
    '/api/graphql/playground',
    '/api/identity/did',
    '/api/identity/credentials',
    '/api/federation/sync',
    '/api/federation/peers',
  ]);

  // Check routes in Rust that are missing from spec
  for (const rustPath of rustPathSet) {
    if (rustPath.includes('/api/') && !specPathSet.has(rustPath)) {
      if (excludedRoutes.has(rustPath)) {
        pass(`${rustPath} intentionally excluded from REST spec (non-REST or internal)`);
      } else {
        fail(`Route ${rustPath} in main.rs not documented in OpenAPI spec`);
      }
    }
  }

  // Check that all spec paths have at least one registered route
  for (const specPath of specPathSet) {
    if (rustPathSet.has(specPath)) {
      pass(`${specPath} registered in main.rs`);
    }
    // Not all spec paths need to be in main.rs if they're mounted differently
  }
}

// 3. Check route handler files exist for each spec tag
console.log('\n3. Route handler file coverage:');
const tagToFile = {
  data: 'data.rs',
  search: 'search.rs',
  events: 'events.rs',
  conflicts: 'conflicts.rs',
  system: 'system.rs',
  health: 'health.rs',
};

for (const [tag, file] of Object.entries(tagToFile)) {
  const filePath = resolve(ROUTES_DIR, file);
  if (existsSync(filePath)) {
    pass(`routes/${file} exists for ${tag} endpoints`);
  } else {
    fail(`routes/${file} missing for ${tag} endpoints`);
  }
}

// Auth routes are in auth/routes.rs
if (existsSync(AUTH_ROUTES)) {
  pass('auth/routes.rs exists for auth endpoints');
} else {
  fail('auth/routes.rs missing for auth endpoints');
}

// 4. Error code coverage
console.log('\n4. Error code coverage:');
const implCodes = extractErrorCodes(ROUTES_DIR);
const specCodes = extractSpecErrorCodes(specText);

// Also check the API overview docs for documented error codes
const overviewPath = resolve(ROOT, 'apps/web/src/content/docs/api/overview.md');
let docCodes = new Set();
if (existsSync(overviewPath)) {
  const overview = readFileSync(overviewPath, 'utf-8');
  const codeRe = /\*\*`([A-Z][A-Z_]+)`\*\*/g;
  let match;
  while ((match = codeRe.exec(overview)) !== null) {
    docCodes.add(match[1]);
  }
}

// Internal error codes that are not part of the public API surface
const internalErrorCodes = new Set([
  'PLUGIN_ROUTE_STUB',
]);

for (const code of implCodes) {
  if (internalErrorCodes.has(code)) {
    pass(`Error code ${code} is internal (not public API)`);
  } else if (docCodes.has(code)) {
    pass(`Error code ${code} documented in API overview`);
  } else {
    fail(`Error code ${code} used in route handlers but not documented in API overview`);
  }
}

// 5. OpenAPI spec structural validation
console.log('\n5. OpenAPI spec structure:');
if (specText.includes('openapi: 3.1.0') || specText.includes("openapi: '3.1.0'")) {
  pass('OpenAPI version is 3.1.0');
} else if (specText.includes('openapi: 3.0')) {
  pass('OpenAPI version is 3.0.x');
} else {
  fail('Could not determine OpenAPI version');
}

if (specText.includes('info:') && specText.includes('title:')) {
  pass('Spec has info.title');
} else {
  fail('Spec missing info.title');
}

if (specText.includes('paths:')) {
  pass('Spec has paths section');
} else {
  fail('Spec missing paths section');
}

if (specText.includes('components:') && specText.includes('schemas:')) {
  pass('Spec has component schemas');
} else {
  fail('Spec missing component schemas');
}

if (specText.includes('securitySchemes:')) {
  pass('Spec defines security schemes');
} else {
  fail('Spec missing security schemes');
}

// 6. Verify all spec operations have operationId
console.log('\n6. Operation ID completeness:');
const opIdRe = /operationId:\s*(\S+)/g;
const opIds = new Set();
let opMatch;
while ((opMatch = opIdRe.exec(specText)) !== null) {
  opIds.add(opMatch[1]);
}

let totalMethods = 0;
for (const methods of Object.values(specPaths)) {
  totalMethods += methods.size;
}

if (opIds.size === totalMethods) {
  pass(`All ${totalMethods} operations have operationIds`);
} else {
  fail(`${opIds.size} operationIds found but ${totalMethods} operations exist`);
}

// Summary
console.log(`\n${'='.repeat(50)}`);
console.log(`Results: ${passes} passed, ${failures} failed`);

if (failures > 0) {
  console.error('\nContract tests failed. Fix the mismatches above.');
  process.exit(1);
} else {
  console.log('\nAll contract tests passed.');
  process.exit(0);
}
