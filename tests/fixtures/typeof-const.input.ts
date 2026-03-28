// Pattern 1: const string array + [number] → string enum
const ENCODING_TYPES = ['gzip', 'deflate'] as const;

interface CompressionOptions {
  encoding: (typeof ENCODING_TYPES)[number];
  threshold: number;
}

// Pattern 2: const object with number values + [keyof typeof X] → value type
const Phase = {
  Stringify: 1,
  BeforeStream: 2,
  Stream: 3,
} as const;

type PhaseValue = (typeof Phase)[keyof typeof Phase];

// Pattern 3: const object with string values + [keyof typeof X] → string enum
const Mimes = {
  aac: 'audio/aac',
  avi: 'video/avi',
} as const;

type MimeType = (typeof Mimes)[keyof typeof Mimes];

// Pattern 4: keyof typeof for Record keys
const detectors = {
  querystring: true,
  cookie: true,
  header: true,
} as const;

type DetectorKeys = keyof typeof detectors;
