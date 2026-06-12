# Extensibility: A Conceptual White Paper

## 1. Extensibility as a Design Axis

The system is not a monolith with optional knobs. It is a composition of three pluggable subsystems, each addressing a distinct concern in the privacy pipeline:

- **Codecs** determine which formats the system understands.
- **Recognizers** determine which categories of sensitive entities it detects.
- **Operators** determine how those entities are transformed once detected.

These three surfaces are orthogonal. Adding a recognizer does not require changes to any codec. Adding an operator does not require changes to any recognizer. Adding a codec does not change the contract that recognizers and operators see. The independence is structural, not aspirational: each surface is a registry of contracts, and the pipeline composes contracts at runtime rather than embedding a fixed set of capabilities at compile time.

This paper describes the three extension surfaces as conceptual contracts. It is not an API reference. The reader will leave with a model of what an extension is, what it must provide, what the system provides in return, and where the boundaries of the model lie. Build mechanics, repository layout, and the contribution workflow live in the project's contribution guide; they are deliberately omitted here.

The intended audience is twofold. The first reader is an engineer evaluating whether the system can be extended to a particular use case — a new document format, a regulatory category not yet covered by the default recognizers, an in-house redaction policy. The second reader is a designer interested in the extension surfaces themselves, perhaps to compare against alternative architectures. The paper is written so that neither reader requires familiarity with the codebase; the architectural argument is meant to stand on its own.

```
                     +---------------+
                     |   Pipeline    |
                     +-------+-------+
                             |
        +--------------------+--------------------+
        |                    |                    |
   +----v-----+        +-----v-----+        +-----v-----+
   |  Codec   |        | Recognizer|        |  Operator |
   | (format) |        |  (detect) |        | (transform)|
   +----------+        +-----------+        +-----------+
        ^                    ^                    ^
        |                    |                    |
   "what bytes         "what entities       "what happens
    we can read"        we can find"         to them"
```

Each surface answers one question. The questions do not overlap, and that is the property the architecture exists to preserve.

A useful frame: the pipeline is a producer–consumer chain in which a codec produces handles for recognizers and operators to consume, recognizers produce detections for operators to consume, and operators produce mutations for the codec to consume on the way back out. The three surfaces are arranged around this chain, each at a distinct seam. An extension joins the system by attaching at one seam and only at that seam; it does not reach across the chain to influence the others.

The implication for someone evaluating the system is direct. The cost of an extension is bounded by the surface it inhabits. A new entity category is cheap because the surface is narrow. A new format is expensive because the surface is wide. The architecture does not equalize these costs — it names them, and it ensures that the wide cost of one surface never silently leaks into the narrow cost of another.

## 2. The Format Extension Surface

A format extension teaches the system how to read and write a new content type. It is the most invasive of the three surfaces because it touches the deepest contract: the representation of content as a typed handle that the rest of the pipeline operates on.

A new format is defined by two complementary contracts:

- A **decoder** consumes a raw byte stream and produces a typed handle suitable for downstream operations.
- An **encoder** consumes a (possibly mutated) handle and produces a byte stream that can be persisted or returned.

The handle is not a passive container. It must support four behaviors that the pipeline relies on:

1. **Streaming chunking.** The handle yields its content as a sequence of chunks. Large inputs are never required to fit in memory in full.
2. **Random-access reads.** A chunk can be revisited; recognizers that need to look backward or forward are not forced to buffer.
3. **In-place mutation.** When an operator decides to replace a region, the handle accepts the replacement without requiring the caller to reconstruct the document.
4. **Coordinate lifting.** A region detected inside a chunk carries chunk-local coordinates. The handle must translate these into source-level coordinates that survive across reads, mutations, and re-encoding.

The format extension registers itself under a stable identifier and a set of lookup keys: file extensions, media types, and any other discriminator the deployment uses to route content. Resolution at decode time is registry-driven.

The contract acknowledges a trade-off. For formats with byte-level structure — plain text, delimited tabular data, line-oriented logs — the contract is straightforward: source coordinates and byte coordinates align, and mutations replay through encode without surprises. For deeply-parsed formats — structured markup, office documents, container formats — source-byte fidelity is unavoidably lost. The handle is the source of truth; the original bytes are not reconstructable after encode. The system documents this rather than pretending otherwise.

A format extension is the only surface that must engage with all four handle behaviors. The remaining surfaces consume the handle and never construct it.

A second consequence follows from this asymmetry. Because the handle abstraction is the contract every downstream component sees, a format extension is the only place where misimplementation can corrupt every other component's view of the world. A recognizer that reports the wrong offset misredacts one detection. A handle that lifts coordinates incorrectly misredacts every detection that flows through it. The system mitigates this with a coordinate-lifting contract that is small in surface but precise in semantics; the burden on the format author is to honor that contract under all chunking strategies the author chooses to implement.

```
   raw bytes  --decoder-->  handle  --encoder-->  raw bytes'
                              |
                              +-- streaming chunking
                              +-- random-access reads
                              +-- in-place mutation
                              +-- coordinate lifting
```

The diagram is not incidental. The handle is the durable artifact across the pipeline's lifetime; the bytes on either side are transient.

## 3. The Recognizer Extension Surface

A recognizer answers a single question: given a region of content, what entities does it contain? It is, in conceptual terms, a function from a payload to a set of detections, where each detection carries:

- a **kind** (the category of entity discovered, drawn from an open vocabulary),
- a **provenance** (where it came from in the source coordinates),
- a **confidence** (how strongly the recognizer asserts the finding).

Recognizers are typed by modality — a text recognizer cannot be applied to tabular content and vice versa — but new recognizers can be added without modifying the registry's definition. The registry is an open type-map keyed by the modality's identity. The system does not need to know in advance how many recognizers will eventually be registered, nor what they detect; at pipeline time it fans detection out across every registered recognizer that applies and unions the results.

This surface is the least invasive of the three. A pattern-driven recognizer for a new entity category — a regional identifier number, a domain-specific token — is implementable in isolation. The author writes the detection logic, declares the entity kinds it produces, and registers it. There is one system-level obligation: the recognizer must honor the chunk-relative coordinate contract. A detection reported at the wrong offset will be redacted in the wrong place; the system cannot detect this for the author.

Recognizers that depend on external inference — named-entity recognition models, language models, classifiers backed by remote services — are subject to the same contract. The recognizer is the abstraction; how it produces its answers is opaque to the registry.

Because the registry is open and the fan-out is parallel, the cost model for adding a recognizer is additive rather than multiplicative. Two recognizers detecting overlapping kinds do not interfere: the union of their detections is reconciled by the pipeline, not by the recognizers. This is the basis on which a deployment can stack a high-precision pattern-driven recognizer alongside a high-recall model-driven recognizer for the same category, without either author needing to be aware of the other.

The contract leaves one judgment to the recognizer: confidence calibration. The system uses confidence to reconcile competing detections; a recognizer that reports an over-confident answer will dominate, and a recognizer that under-reports will be ignored. The architecture does not normalize confidence across recognizers, and it makes no claim to. The judgment is a deployment concern, expressed in the recognizers the deployment chooses to register.

The cost model for this surface is the architectural payoff of the paper. A new recognizer is conceptually a single function, registered against the entity kinds it asserts. There is no protocol negotiation, no compatibility table, no lifecycle hook beyond registration. This is the surface to reach for first when a deployment asks how the system can be tailored.

## 4. The Operator Extension Surface

An operator takes a detection and returns a replacement. It is the surface that determines what redaction actually looks like.

Operators, like recognizers, are typed by modality. They are registered against the entity kinds they handle. A new operator — a tokenizer that produces stable pseudonyms for a particular entity category, a hash-based blinder for a class of identifiers, a generator that produces realistic synthetic substitutes — implements the per-modality replacement contract and declares which kinds it serves.

Two operator dispositions are first-class:

- **Irreversible.** The replacement discards information; the original cannot be recovered. Masking, deletion, and one-way hashing fall in this category.
- **Reversible.** The operator participates in a paired protocol, registering both as the forward transform and as its inverse. Shared state — a vault, a deterministic key, a mapping table — is the operator's responsibility, not the pipeline's. The pipeline guarantees that the inverse, when invoked, sees the same registry and the same content shape as the forward pass.

The pipeline does not adjudicate between dispositions. An operator declares what it is, and the deployment configures which operators apply to which kinds. The registry composes; the pipeline executes.

An additional property is worth naming: operators are pure with respect to the handle. They observe a detection, they emit a replacement, and the handle applies the replacement. They do not mutate the handle directly. This separation is what makes operators substitutable. A deployment can swap a masking operator for a tokenizing operator for a given entity kind, and the pipeline composes the change without any downstream coordination. The handle is the single mutator; operators describe mutations rather than performing them.

A consequence for reversible operators is worth surfacing. Because forward and inverse share state but not control flow — they are independent registrations — the operator author owns the state's lifecycle. The pipeline does not provide a vault, a key store, or a mapping table; it provides the seams at which such things attach. This is a deliberate refusal: state management for reversible redaction is a security-sensitive concern, and the system does not presume a one-size policy for it.

## 5. The Modality Boundary

The three extension surfaces are open. The set of modalities they range over is not.

Modalities — text, tabular, image, audio — form a closed set in the current architecture. Each modality has its own coordinate space, its own notion of what a chunk is, its own definition of what a redactable region looks like. The modality identity participates in the type system: registries are keyed by it, contracts are parameterized by it, and the compiler enforces that a recognizer or operator written for one modality cannot be misapplied to another.

This is a deliberate trade-off. Adding a fifth modality is not a plug-in operation. It is a workspace-wide change: a new coordinate type, a new chunk contract, new operator contracts, new format handlers, and updates to every component that pattern-matches on the modality set. The system buys type-safety invariants — the impossibility of routing image bytes through a text operator — at the cost of extensibility along this axis.

The cost is real and is named here rather than glossed over. Deployments that need a modality outside the closed set will not find a quick path. The judgment underlying the closed set is that the type-system guarantees pay for themselves across the rest of the surface area, where extension is plentiful and safe.

```
   open extension axes          closed axis
   ------------------          --------------
   codecs:        many         modalities:
   recognizers:   many             text
   operators:     many             tabular
   backends:     several           image
                                   audio
```

The contrast is the central architectural fact of the system. The open axes are where day-to-day extensibility lives. The closed axis is where the type system enforces global coherence. A reader weighing whether the system fits their use case should map the use case onto this contrast: extensions that align with the open axes are routine; extensions that cross the closed axis are projects.

## 6. Pluggable Inference Backends

Recognizers that depend on machine inference — entity-recognition models, language models, speech-to-text, optical recognition — abstract over the source of inference behind a backend interface. The recognizer asks; the backend answers; the recognizer interprets.

The consequence is that a deployment can change the inference substrate without changing detection logic. A site that migrates from one language-model provider to another, or replaces a hosted model with an on-premise inference server, or substitutes one entity-recognition model for another, updates configuration and not code. The system ships with backends for common deployments. These are defaults, not requirements; a deployment is free to supply its own.

The backend interface is itself an extension surface, narrower than the three principal surfaces but governed by the same principle: the system commits to a contract, the deployment supplies an implementation, and the registry mediates.

The narrowness is intentional. A backend is a leaf: it does not see the pipeline, it does not see the registry, it does not see other backends. It sees only the requests its recognizer sends and the answers it returns. This isolation is what makes a backend swap safe. The site of substitution is small enough to reason about; the contract is small enough to verify. A deployment that needs to comply with data-residency constraints can choose backends accordingly without auditing the rest of the system.

## 7. What the Architecture Does Not Promise

Honest description requires naming what extensibility does not mean here.

- **There is no no-code extension surface.** Adding a format, a recognizer, or an operator requires writing code in the host language. The system does not offer a configuration-driven path for any of the three surfaces, and there is no plan to introduce one.
- **There is no runtime loading of extensions.** All extensions are compiled into the binary. A deployment that needs a new extension produces a new binary. This is the cost of the type-system guarantees that the rest of the architecture is built on.
- **There is no backwards-compatibility guarantee across major contract revisions.** The system reserves the right to evolve the extension contracts. Extensions written against an earlier major version are expected to update. Deprecation cycles for minor changes exist; soft-deprecation paths across major boundaries do not.

These are trade-offs, not oversights. Each is a deliberate exchange of one form of flexibility for one form of predictability.

The honest framing is that the architecture optimizes for deployments that own their build pipeline and can produce binaries. A deployment that must consume third-party plug-ins drawn from an open ecosystem at runtime is not the deployment the system is designed for. A deployment that builds, tests, and ships a curated set of extensions as part of its operational practice is exactly the deployment the system is designed for. Prospective adopters should locate themselves on this spectrum before judging the trade-off.

## 8. The Extension Lifecycle

An extension proceeds through four stages, regardless of which surface it inhabits.

```
   identify  -->  contract  -->  register  -->  verify
   "what is      "implement     "tell the      "pass the
    this in      the required    registry       four gates"
    the          interface"      it exists"
    system's
    vocabulary?"
```

1. **Conceptual identification.** The author names what the extension is in the system's vocabulary. Is it a new format — a new way of representing content? A new recognizer family — a new class of entity to detect? A new operator category — a new transformation to apply? The answer determines the surface.

2. **Contract implementation.** The author implements the contracts the chosen surface requires. The contracts are not negotiable. The pipeline depends on them being honored; failures here propagate.

3. **Registration.** The extension is registered with the appropriate registry during startup. Registration is explicit. The system does not discover extensions implicitly; what is not registered does not exist as far as the pipeline is concerned.

4. **Verification.** The system runs a four-gate verification across every change: build, lint, the unit and integration test suite, and documentation. An extension is not optional with respect to these gates. They apply equally to first-party and third-party code, and they apply equally to the pipeline's core and to any extension added to it.

The lifecycle establishes that extensions are first-class members of the system. They are not afterthoughts attached to a frozen core; they are how the system grows.

Two properties of the lifecycle are worth emphasizing for a prospective extension author. First, the four gates are symmetric: the same gates that the system applies to its own internals apply to extensions, with no second-class status for either. There is no internal-only test suite that an extension is exempt from, and there is no relaxed lint posture for newly added code. Second, the gates are pre-merge, not post-merge. The system does not accept code that fails a gate on the promise of a follow-up fix. The intent is that the main line of the codebase is, at every commit, in a shippable state — extensions included.

## 9. The Contributor Compact

The system carries conventions that govern how it evolves. They are not specific to extension authors, but extension authors inherit them.

- **Conditional compilation is minimized.** Code paths branch on configuration, not on compile-time switches, except in narrow cases where the alternative is worse.
- **There are no soft-deprecation paths inside the codebase.** An extension is present and supported, or it is removed. Half-removed code is not a stable state.
- **Suppressed warnings are not tolerated.** The build is clean. Warnings are addressed; they are not silenced. An extension that requires suppression has, in the project's judgment, not yet been written correctly.
- **Documentation moves with code.** A contract change without a corresponding documentation change fails verification. Documentation is part of the build, not adjacent to it.

These conventions are stylistic. They are not gates intended to discourage contribution. They are the form discipline takes here, and the form the codebase will continue to take. An extension that respects them inherits the system's stability guarantees; one that does not will be reshaped until it does.

There is a deeper reason for the compact. The system runs in environments where privacy guarantees are operationally consequential — a redaction that silently fails has real cost. The conventions are not aesthetic preferences imported from elsewhere; they are the codebase's defense against the class of bug that a redaction system is least equipped to tolerate. An extension that lowers the standard locally raises the operational risk globally. The conventions distribute the defense across every line of code, including code that has not been written yet.

## 10. Closing Note

Extensibility in this system is local where it can be — codecs, recognizers, operators — and global where the type system insists — modalities. The architecture trades freedom along one axis for safety along three others. The contracts are sharp on purpose; the registries are open on purpose; the modality boundary is closed on purpose.

The reader evaluating the system should take away three claims. First, that the surfaces are independent, and that this independence is structural rather than nominal. Second, that the modality boundary is the one place the independence breaks down, and that the system is candid about the cost. Third, that the verification and convention apparatus around extensions is uniform: extensions are not second-class. Together, these claims define the system the prospective contributor will be working with.

A reader who has reached this point should have a working model of where the system invites extension, where it does not, and what the cost of each invitation is. The detail of any particular contract is a matter for the corresponding interface documentation; the conceptual map is the contribution of this paper.
