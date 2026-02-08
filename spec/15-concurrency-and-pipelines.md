# 15 - Concurrency and Pipelines

## Overview

AI orchestration requires concurrent execution -- LLM calls are inherently async and often independent tasks can run in parallel. Concerto provides `async`/`await` for asynchronous execution, parallel await for concurrent resolution, and first-class `pipeline`/`stage` constructs for declarative multi-agent workflows.

## Async / Await

All LLM-related operations (agent calls, tool executions, bidirectional emits) are async. The `async`/`await` model prevents blocking the runtime.

### Async Functions

```concerto
async fn classify(doc: String) -> Result<Classification, AgentError> {
    let response = Classifier.execute_with_schema<Classification>(doc).await?;
    Ok(response)
}
```

### Await

The `.await` keyword suspends execution until the async operation completes:

```concerto
async fn main() {
    let result = classify("Some document").await?;
    emit("result", result);
}
```

### Sequential Await

Each `.await` waits for completion before proceeding:

```concerto
async fn sequential_pipeline(doc: String) -> Result<String, AgentError> {
    // Each step waits for the previous
    let extracted = Extractor.execute(doc).await?;
    let classified = Classifier.execute(extracted.text).await?;
    let summarized = Summarizer.execute(classified.text).await?;
    Ok(summarized.text)
}
```

## Parallel Execution

### Parallel Await (Tuple)

Execute multiple async operations concurrently and wait for all to complete:

```concerto
async fn parallel_analysis(doc: String) -> Result<Analysis, AgentError> {
    // All three run concurrently
    let (classification, sentiment, entities) = await (
        Classifier.execute_with_schema<Classification>(doc),
        SentimentAnalyzer.execute_with_schema<Sentiment>(doc),
        EntityExtractor.execute_with_schema<Entities>(doc),
    );

    // All three have completed
    Ok(Analysis {
        classification: classification?,
        sentiment: sentiment?,
        entities: entities?,
    })
}
```

### Parallel For

Execute the same operation across a collection concurrently:

```concerto
async fn batch_classify(docs: Array<String>) -> Array<Result<Classification, AgentError>> {
    parallel for doc in docs {
        Classifier.execute_with_schema<Classification>(doc).await
    }
}
```

`parallel for` returns an `Array` of results in the same order as the input. The runtime manages concurrency limits based on connection rate limits.

### First Completed (Race)

Wait for the first of several async operations to complete:

```concerto
async fn fastest_response(prompt: String) -> Result<Response, AgentError> {
    let winner = race (
        FastAgent.execute(prompt),
        ReliableAgent.execute(prompt),
    );
    winner
}
```

`race` returns the result of the first operation to complete. Remaining operations are cancelled.

## Pipeline Construct

Pipelines are first-class constructs for defining multi-step agent workflows. They provide structure, automatic data passing between stages, and per-stage error handling.

### Pipeline Definition

```concerto
pipeline PipelineName {
    stage stage_name(input: InputType) -> OutputType {
        // stage body
        // last expression is the stage output
    }

    stage next_stage(prev_output: OutputType) -> NextOutputType {
        // receives output from previous stage
    }
}
```

### Basic Pipeline

```concerto
pipeline DocumentProcessor {
    stage extract(doc: String) -> String {
        let response = Extractor.execute(doc).await?;
        response.text
    }

    stage classify(text: String) -> Classification {
        Classifier.execute_with_schema<Classification>(text).await?
    }

    stage summarize(classification: Classification) -> String {
        let prompt = "Summarize this ${classification.label} document: ${classification.reasoning}";
        let response = Summarizer.execute(prompt).await?;
        response.text
    }
}

// Execute the pipeline
async fn main() {
    let result = DocumentProcessor.run("Raw document text...").await?;
    emit("summary", result);
}
```

### Pipeline Execution

```concerto
// Run the full pipeline
let result = MyPipeline.run(initial_input).await?;

// The pipeline executes stages sequentially:
// 1. extract(initial_input) -> text
// 2. classify(text) -> classification
// 3. summarize(classification) -> summary
// 4. Returns the final stage's output
```

### Stage Data Flow

Each stage's output becomes the next stage's input. Types must match:

```concerto
pipeline TypedPipeline {
    stage a(input: String) -> Int {      // String -> Int
        input.len()
    }

    stage b(count: Int) -> Float {       // Int -> Float
        count as Float * 1.5
    }

    stage c(value: Float) -> String {    // Float -> String
        "Result: ${value}"
    }
}
// Pipeline type: String -> String (input of first, output of last)
```

### Branching Pipelines

Stages can branch based on intermediate results:

```concerto
pipeline RoutingPipeline {
    stage analyze(doc: String) -> Analysis {
        Analyzer.execute_with_schema<Analysis>(doc).await?
    }

    stage route(analysis: Analysis) -> String {
        match analysis.category {
            "legal" => {
                let result = LegalAgent.execute(analysis.content).await?;
                result.text
            },
            "technical" => {
                let result = TechAgent.execute(analysis.content).await?;
                result.text
            },
            "financial" => {
                let result = FinanceAgent.execute(analysis.content).await?;
                result.text
            },
            _ => {
                let result = GeneralAgent.execute(analysis.content).await?;
                result.text
            },
        }
    }

    stage format(response: String) -> FormattedOutput {
        Formatter.execute_with_schema<FormattedOutput>(response).await?
    }
}
```

### Pipeline with Memory

Pipelines can use databases to maintain state across stages:

```concerto
db pipeline_state: Database<String, Any> = Database::new();

pipeline StatefulPipeline {
    stage step_one(input: String) -> String {
        let result = AgentA.execute(input).await?;
        pipeline_state.set("step_one_output", result.text);
        pipeline_state.set("step_one_time", std::time::now());
        result.text
    }

    stage step_two(prev: String) -> String {
        // Can read state from previous stages
        let step_one_time = pipeline_state.get("step_one_time");
        let result = AgentB.execute(prev).await?;
        pipeline_state.set("step_two_output", result.text);
        result.text
    }
}
```

### Stage Decorators

#### Per-Stage Timeout

```concerto
pipeline TimedPipeline {
    @timeout(seconds: 10)
    stage fast_step(input: String) -> String {
        QuickAgent.execute(input).await?
    }

    @timeout(seconds: 120)
    stage slow_step(prev: String) -> String {
        DeepAnalyzer.execute(prev).await?
    }
}
```

#### Per-Stage Retry

```concerto
pipeline ReliablePipeline {
    @retry(max: 3, backoff: "exponential")
    stage unreliable_step(input: String) -> String {
        ExternalAgent.execute(input).await?
    }
}
```

### Pipeline Error Handling

#### Stage-Level Error Recovery

```concerto
pipeline ResilientPipeline {
    stage primary(input: String) -> String {
        match PrimaryAgent.execute(input).await {
            Ok(response) => response.text,
            Err(_) => {
                emit("warning", "Primary agent failed, using fallback");
                let fallback = FallbackAgent.execute(input).await?;
                fallback.text
            },
        }
    }
}
```

#### Pipeline-Level Error Handling

```concerto
async fn main() {
    match DocumentProcessor.run(input).await {
        Ok(result) => emit("result", result),
        Err(e) => {
            emit("pipeline_error", {
                "pipeline": "DocumentProcessor",
                "error": e.message,
            });
        },
    }
}
```

### Pipeline Events

Pipelines automatically emit stage lifecycle events:

```concerto
// These are emitted automatically by the runtime:
// emit("pipeline:start", { "name": "DocumentProcessor", "input": ... })
// emit("pipeline:stage_start", { "name": "DocumentProcessor", "stage": "extract" })
// emit("pipeline:stage_complete", { "name": "DocumentProcessor", "stage": "extract", "duration_ms": 1200 })
// emit("pipeline:stage_start", { "name": "DocumentProcessor", "stage": "classify" })
// ...
// emit("pipeline:complete", { "name": "DocumentProcessor", "duration_ms": 4500 })
// OR
// emit("pipeline:error", { "name": "DocumentProcessor", "stage": "classify", "error": "..." })
```

## Rate Limiting

The runtime enforces rate limits defined in connection configurations. When concurrent operations would exceed limits, they are automatically queued:

```concerto
connect openai {
    api_key: env("OPENAI_API_KEY"),
    rate_limit: {
        requests_per_minute: 60,
        tokens_per_minute: 150000,
        concurrent_requests: 10,
    },
}

// Even with parallel for, the runtime respects rate limits
async fn batch_process(docs: Array<String>) {
    let results = parallel for doc in docs {
        // Runtime queues requests to stay within 60 RPM / 10 concurrent
        Classifier.execute(doc).await
    };
}
```

## Concurrency Summary

| Pattern | Syntax | Use Case |
|---------|--------|----------|
| Sequential await | `a.await; b.await;` | Dependent operations |
| Parallel await | `await (a, b, c)` | Independent operations, wait for all |
| Parallel for | `parallel for x in items { ... }` | Same operation on many items |
| Race | `race (a, b)` | First to complete wins |
| Pipeline | `pipeline { stage a {...} stage b {...} }` | Multi-step workflows |
| Pipeline run | `Pipeline.run(input).await` | Execute a pipeline |
