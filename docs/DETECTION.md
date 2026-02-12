# Sensitive Data Detection

## 1. Overview

The detection engine is the core intellectual property of the platform. It is responsible for identifying sensitive content across all supported modalities with high precision and recall. Detection must operate through multiple complementary strategies — deterministic pattern matching, learned models, and computer vision — to achieve robust coverage across diverse content types and regulatory categories.

## 2. Language Coverage

The platform must support detection across multiple languages and writing systems. Real-world data frequently contains non-English text, multilingual documents, and code-switched content (multiple languages within a single document or conversation). Detection models must handle at minimum the major European languages, CJK (Chinese, Japanese, Korean), and Arabic script. Deterministic patterns must be parameterized by locale — national identifier formats, date conventions, and address structures vary by jurisdiction.

For audio, speech-to-text and subsequent NER must support the same language set, including language identification and mid-utterance language switching.

## 3. Deterministic Detection

Deterministic methods provide high-precision, low-latency detection for well-defined patterns:

- **Regular expressions**: Pattern matching for structured identifiers such as Social Security numbers, credit card numbers, passport numbers, and other nationally defined formats.
- **Checksum validation**: Algorithmic verification (e.g., Luhn algorithm for credit card numbers) to reduce false positives from pattern matching alone.
- **Custom pattern libraries**: User-defined pattern sets that extend detection to organization-specific sensitive categories such as internal project identifiers, proprietary terms, or custom reference numbers.

## 4. Machine Learning and NLP-Based Detection

Learned models address the detection of sensitive content that cannot be captured by fixed patterns:

- **Named entity recognition (NER)**: Identification of person names, locations, organizations, and other entity types in unstructured text.
- **Domain-specific entity models**: Specialized models trained on financial data, medical records (HIPAA-relevant entities), legal identifiers, and biometric references.
- **Contextual detection**: Inference of sensitivity from surrounding context rather than explicit entity presence. Phrases such as "the patient" or "my lawyer" may indicate sensitive content even in the absence of a named entity. This capability requires models that reason over discourse context rather than isolated tokens.

## 5. Computer Vision Detection

Visual content requires detection methods that operate on pixel-level and spatial features:

- **Face detection and recognition**: Identification of human faces in images and video frames for subsequent obfuscation.
- **Document and identifier detection**: Recognition of identity documents, license plates, and other visual identifiers.
- **Handwritten text detection**: Extraction and analysis of handwritten content in scanned documents and images.
- **Screen capture analysis**: Detection of sensitive text rendered in screenshots, application windows, and other digital captures.

## 6. Audio Detection

Audio content introduces temporal and speaker-level dimensions to detection:

- **Transcript-based NER**: Application of named entity recognition to speech-to-text output, with alignment back to audio timestamps.
- **Direct waveform redaction**: Replacement of sensitive audio segments with silence, tones, or noise at the waveform level.
- **Speaker-specific redaction**: Selective redaction of content from identified speakers while preserving contributions from others, enabled by speaker diarization.

## 7. Detection Orchestration

Individual detection strategies — deterministic, ML-based, vision, and audio — must be composed into a coherent pipeline rather than operating in isolation.

### 7.1 Tiered Execution

Detection should proceed in tiers ordered by cost and specificity. Deterministic patterns (regex, checksums) execute first, providing high-precision results at minimal computational cost. ML and vision models execute subsequently, targeting content that deterministic methods cannot address. This tiered architecture avoids unnecessary GPU inference for content that can be resolved through pattern matching alone.

### 7.2 Result Merging

When multiple detection strategies identify overlapping or adjacent sensitive regions within the same content, the platform must merge results into a unified set of detection annotations. Overlapping detections should be consolidated rather than duplicated. Each merged annotation must retain provenance — which strategies contributed to the detection and at what confidence level.

### 7.3 Conflict Resolution

When detection strategies disagree — for example, a regex match identifies a number as a credit card while an NER model classifies the surrounding context as non-sensitive — the platform must apply configurable conflict resolution rules. Default behavior should favor the higher-confidence or higher-sensitivity classification, but administrators must be able to override this through policy.
