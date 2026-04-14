/**
 * tsc type extraction script.
 *
 * Uses the TypeScript Compiler API to extract fully-resolved type information
 * from TypeScript source files or declaration files (lib.dom.d.ts, etc.).
 *
 * Usage:
 *   node dist/index.js --lib dom,webworker          # Extract from TS built-in libs
 *   node dist/index.js --tsconfig ./tsconfig.json    # Extract from a project
 *   node dist/index.js --files a.d.ts b.d.ts         # Extract from specific files
 */

import ts from "typescript";
import { extractTypes } from "./extractor.js";
import { filterTypes, SERVER_WEB_API_TYPES, ECMASCRIPT_TYPES } from "./filter.js";

function main(): void {
  const args = process.argv.slice(2);
  const serverWebApi = args.includes("--server-web-api");
  const ecmascript = args.includes("--ecmascript");

  if (serverWebApi && ecmascript) {
    console.error("--server-web-api and --ecmascript are mutually exclusive");
    process.exit(1);
  }

  let program: ts.Program;

  if (args.includes("--lib")) {
    const libIdx = args.indexOf("--lib");
    const libs = (args[libIdx + 1] ?? "dom").split(",");
    program = createLibProgram(libs);
  } else if (args.includes("--tsconfig")) {
    const configIdx = args.indexOf("--tsconfig");
    const configPath = args[configIdx + 1];
    if (!configPath) {
      console.error("--tsconfig requires a path argument");
      process.exit(1);
    }
    program = createProjectProgram(configPath);
  } else if (args.includes("--files")) {
    const filesIdx = args.indexOf("--files");
    const files = args.slice(filesIdx + 1);
    if (files.length === 0) {
      console.error("--files requires at least one file path");
      process.exit(1);
    }
    program = createFilesProgram(files);
  } else {
    console.error(
      "Usage: extract-types --lib dom,webworker | --tsconfig path | --files file1.d.ts ...",
    );
    process.exit(1);
  }

  let result = extractTypes(program);
  if (serverWebApi) {
    result = filterTypes(result, SERVER_WEB_API_TYPES);
  } else if (ecmascript) {
    result = filterTypes(result, ECMASCRIPT_TYPES);
  }
  process.stdout.write(JSON.stringify(result, null, 2));
}

/** Creates a program with TypeScript built-in lib files. */
function createLibProgram(libs: string[]): ts.Program {
  const libMap: Record<string, string> = {
    dom: "lib.dom.d.ts",
    webworker: "lib.webworker.d.ts",
    es2024: "lib.es2024.d.ts",
    es5: "lib.es5.d.ts",
    "es2015.core": "lib.es2015.core.d.ts",
    "es2015.collection": "lib.es2015.collection.d.ts",
    "es2015.symbol": "lib.es2015.symbol.d.ts",
    "es2015.symbol.wellknown": "lib.es2015.symbol.wellknown.d.ts",
    "es2015.promise": "lib.es2015.promise.d.ts",
    "es2015.iterable": "lib.es2015.iterable.d.ts",
    "es2015.generator": "lib.es2015.generator.d.ts",
    "es2015.proxy": "lib.es2015.proxy.d.ts",
    "es2015.reflect": "lib.es2015.reflect.d.ts",
  };

  const libFiles = libs.map((lib) => {
    const mapped = libMap[lib];
    if (!mapped) {
      console.error(`Unknown lib: ${lib}. Available: ${Object.keys(libMap).join(", ")}`);
      process.exit(1);
    }
    return mapped;
  });

  // Create a dummy source file that references the libs
  const dummyFileName = "__extract_types_dummy__.ts";
  const dummyContent = "export {};";

  const compilerOptions: ts.CompilerOptions = {
    target: ts.ScriptTarget.ES2024,
    lib: libFiles,
    noEmit: true,
    skipLibCheck: false,
    // strictNullChecks preserves `T | undefined` unions in extracted signatures.
    // Without it, the TypeScript checker simplifies return types such as
    // `find(): T | undefined` back to just `T`, which would prevent the Rust
    // loader from producing `Option<T>`.
    strictNullChecks: true,
  };

  const host = ts.createCompilerHost(compilerOptions);
  const originalGetSourceFile = host.getSourceFile.bind(host);
  host.getSourceFile = (fileName, languageVersion, onError, shouldCreate) => {
    if (fileName === dummyFileName) {
      return ts.createSourceFile(dummyFileName, dummyContent, languageVersion);
    }
    return originalGetSourceFile(fileName, languageVersion, onError, shouldCreate);
  };
  host.fileExists = (fileName) => {
    if (fileName === dummyFileName) return true;
    return ts.sys.fileExists(fileName);
  };
  host.readFile = (fileName) => {
    if (fileName === dummyFileName) return dummyContent;
    return ts.sys.readFile(fileName);
  };

  return ts.createProgram([dummyFileName], compilerOptions, host);
}

/** Creates a program from a tsconfig.json file. */
function createProjectProgram(configPath: string): ts.Program {
  const configFile = ts.readConfigFile(configPath, ts.sys.readFile);
  if (configFile.error) {
    console.error(
      `Error reading ${configPath}: ${ts.flattenDiagnosticMessageText(configFile.error.messageText, "\n")}`,
    );
    process.exit(1);
  }

  const parsedConfig = ts.parseJsonConfigFileContent(
    configFile.config,
    ts.sys,
    configPath.replace(/[/\\][^/\\]+$/, ""),
  );

  return ts.createProgram(parsedConfig.fileNames, {
    ...parsedConfig.options,
    noEmit: true,
    // See createLibProgram: strictNullChecks preserves `T | undefined`.
    // Overrides the project's setting to guarantee consistent extraction.
    strictNullChecks: true,
  });
}

/** Creates a program from explicit file paths. */
function createFilesProgram(files: string[]): ts.Program {
  return ts.createProgram(files, {
    target: ts.ScriptTarget.ES2024,
    noEmit: true,
    skipLibCheck: false,
    // See createLibProgram: strictNullChecks preserves `T | undefined`.
    strictNullChecks: true,
  });
}

main();
