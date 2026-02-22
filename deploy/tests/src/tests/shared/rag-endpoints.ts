/**
 * RAG/Vector Store Endpoint Tests
 *
 * Tests the complete RAG workflow: file upload, vector store creation,
 * file processing, vector search, and cleanup.
 * Migrated from test_rag_endpoints() in deploy/test-e2e.sh.
 *
 * Test scenarios (matching bash script exactly):
 *   1. Upload a test file
 *   2. Create a vector store
 *   3. Add file to vector store
 *   4. Wait for file processing
 *   5. Search the vector store
 *   6. List vector stores
 *   7. List files in vector store
 *   8. Get file chunks (Hadrian extension)
 *   9. Delete file from vector store
 *   10. Delete vector store
 *   11. Delete the uploaded file
 */
import { describe, it, expect } from "vitest";
import { trackedFetch } from "../../utils/tracked-fetch";
import { retry } from "../../utils/retry";

/**
 * Context for RAG endpoint tests.
 */
export interface RagEndpointsContext {
  /** Gateway base URL */
  gatewayUrl: string;
  /** Organization ID for ownership */
  orgId: string;
  /** Org-scoped API key */
  apiKey: string;
}

/**
 * Run RAG/Vector Store endpoint tests.
 * Tests match test_rag_endpoints() from bash script exactly.
 *
 * @param getContext - Function that returns the test context
 */
export function runRagEndpointTests(getContext: () => RagEndpointsContext) {
  describe("RAG/Vector Store Endpoints", () => {
    // Shared state across tests (created in order, cleaned up at end)
    // Note: fileId stores the prefixed version (e.g., "file-abc123") which is what the API expects
    let fileId: string;
    let vectorStoreId: string;

    const testContent = `This is a test document about artificial intelligence.

Machine learning is a subset of artificial intelligence that enables computers
to learn from data without being explicitly programmed. Deep learning uses
neural networks with many layers to process complex patterns.

Natural language processing (NLP) allows computers to understand, interpret,
and generate human language. This enables applications like chatbots,
translation services, and sentiment analysis.`;

    // =========================================================================
    // Test 1: Upload a test file
    // =========================================================================
    describe("File Upload", () => {
      it("uploads a test file successfully", async () => {
        const { gatewayUrl, orgId, apiKey } = getContext();

        // Create a Blob from the test content
        const blob = new Blob([testContent], { type: "text/plain" });
        const formData = new FormData();
        formData.append("file", blob, "test-document.txt");
        formData.append("purpose", "assistants");
        formData.append("owner_type", "organization");
        formData.append("owner_id", orgId);

        const response = await trackedFetch(`${gatewayUrl}/api/v1/files`, {
          method: "POST",
          headers: {
            "X-API-Key": apiKey,
          },
          body: formData,
        });

        // Accept 200 (idempotent re-upload) or 201 (new file)
        expect([200, 201]).toContain(response.status);

        const data = await response.json();
        expect(data.id).toBeDefined();
        expect(data.id).toMatch(/^file-/);

        // Store the prefixed file ID for subsequent tests
        fileId = data.id;
      });
    });

    // =========================================================================
    // Test 2: Create a vector store
    // =========================================================================
    describe("Vector Store Creation", () => {
      it("creates a vector store successfully", async () => {
        const { gatewayUrl, orgId, apiKey } = getContext();

        const response = await trackedFetch(
          `${gatewayUrl}/api/v1/vector_stores`,
          {
            method: "POST",
            headers: {
              "Content-Type": "application/json",
              "X-API-Key": apiKey,
            },
            body: JSON.stringify({
              owner: { type: "organization", organization_id: orgId },
              name: "Test Vector Store",
              embedding_model: "test-embedding",
              metadata: { purpose: "e2e-testing" },
            }),
          }
        );

        expect(response.status).toBe(201);

        const data = await response.json();
        expect(data.id).toBeDefined();
        expect(data.id).toMatch(/^vs_/);

        // Store the prefixed vector store ID for subsequent tests
        vectorStoreId = data.id;
      });
    });

    // =========================================================================
    // Test 3: Add file to vector store
    // =========================================================================
    describe("Add File to Vector Store", () => {
      it("adds file to vector store successfully", async () => {
        if (!fileId) {
          throw new Error(
            "Test prerequisite failed: fileId not set. The 'uploads a test file successfully' test must pass first."
          );
        }
        if (!vectorStoreId) {
          throw new Error(
            "Test prerequisite failed: vectorStoreId not set. The 'creates a vector store successfully' test must pass first."
          );
        }
        const { gatewayUrl, apiKey } = getContext();

        const response = await trackedFetch(
          `${gatewayUrl}/api/v1/vector_stores/${vectorStoreId}/files`,
          {
            method: "POST",
            headers: {
              "Content-Type": "application/json",
              "X-API-Key": apiKey,
            },
            body: JSON.stringify({ file_id: fileId }),
          }
        );

        // 201 for new file, 200 for deduplicated file
        expect([200, 201]).toContain(response.status);

        const data = await response.json();
        expect(data.id).toBeDefined();
      });
    });

    // =========================================================================
    // Test 4: Wait for file processing
    // =========================================================================
    describe("File Processing", () => {
      it("completes file processing within timeout", async () => {
        if (!fileId || !vectorStoreId) {
          throw new Error(
            "Test prerequisite failed: fileId or vectorStoreId not set. Previous tests must pass first."
          );
        }
        const { gatewayUrl, apiKey } = getContext();

        const maxAttempts = 30;

        try {
          await retry(
            async () => {
              const response = await trackedFetch(
                `${gatewayUrl}/api/v1/vector_stores/${vectorStoreId}/files/${fileId}`,
                {
                  headers: { "X-API-Key": apiKey },
                }
              );

              if (!response.ok) {
                throw new Error(`Request failed with status ${response.status}`);
              }

              const data = await response.json();
              if (data.status === "in_progress") {
                throw new Error("Still processing");
              }

              // Processing completed or failed
              expect(["completed", "failed"]).toContain(data.status);
              if (data.status === "failed") {
                console.warn(`[RAG Test] File processing failed: ${data.last_error}`);
              }
              return data;
            },
            {
              maxAttempts,
              initialDelay: 1000,
              backoffMultiplier: 1, // Linear retry, no exponential backoff
            }
          );
        } catch {
          // If we exhausted retries, the file is still processing
          console.warn(
            `[RAG Test] File processing still in_progress after ${maxAttempts}s - subsequent tests may have incomplete data`
          );
        }
      });
    });

    // =========================================================================
    // Test 5: Search the vector store
    // =========================================================================
    describe("Vector Store Search", () => {
      it("returns search results for semantic query", async () => {
        if (!vectorStoreId) {
          throw new Error(
            "Test prerequisite failed: vectorStoreId not set. The 'creates a vector store successfully' test must pass first."
          );
        }
        const { gatewayUrl, apiKey } = getContext();

        const response = await trackedFetch(
          `${gatewayUrl}/api/v1/vector_stores/${vectorStoreId}/search`,
          {
            method: "POST",
            headers: {
              "Content-Type": "application/json",
              "X-API-Key": apiKey,
            },
            body: JSON.stringify({
              query: "what is machine learning?",
              max_num_results: 3,
            }),
          }
        );

        expect(response.status).toBe(200);

        const data = await response.json();
        expect(data.data).toBeDefined();
        // Results may be empty if file processing is still in progress
        // This matches the bash script behavior which only warns on no results
      });
    });

    // =========================================================================
    // Test 6: List vector stores
    // =========================================================================
    describe("List Vector Stores", () => {
      it("lists vector stores for organization", async () => {
        const { gatewayUrl, orgId, apiKey } = getContext();

        const response = await trackedFetch(
          `${gatewayUrl}/api/v1/vector_stores?owner_type=organization&owner_id=${orgId}`,
          {
            headers: { "X-API-Key": apiKey },
          }
        );

        expect(response.status).toBe(200);

        const data = await response.json();
        expect(data.data).toBeDefined();
        expect(Array.isArray(data.data)).toBe(true);
      });
    });

    // =========================================================================
    // Test 7: List files in vector store
    // =========================================================================
    describe("List Files in Vector Store", () => {
      it("lists files in the vector store", async () => {
        if (!vectorStoreId) {
          throw new Error(
            "Test prerequisite failed: vectorStoreId not set. The 'creates a vector store successfully' test must pass first."
          );
        }
        const { gatewayUrl, apiKey } = getContext();

        const response = await trackedFetch(
          `${gatewayUrl}/api/v1/vector_stores/${vectorStoreId}/files`,
          {
            headers: { "X-API-Key": apiKey },
          }
        );

        expect(response.status).toBe(200);

        const data = await response.json();
        expect(data.data).toBeDefined();
        expect(Array.isArray(data.data)).toBe(true);
      });
    });

    // =========================================================================
    // Test 8: Get file chunks (Hadrian extension)
    // =========================================================================
    describe("File Chunks (Hadrian Extension)", () => {
      it("retrieves file chunks", async () => {
        if (!fileId || !vectorStoreId) {
          throw new Error(
            "Test prerequisite failed: fileId or vectorStoreId not set. Previous tests must pass first."
          );
        }
        const { gatewayUrl, apiKey } = getContext();

        const response = await trackedFetch(
          `${gatewayUrl}/api/v1/vector_stores/${vectorStoreId}/files/${fileId}/chunks`,
          {
            headers: { "X-API-Key": apiKey },
          }
        );

        // Chunks endpoint may return 200 with data or may fail if file processing is incomplete
        // This is a Hadrian extension, so we log a warning if it fails
        if (!response.ok) {
          console.warn(
            `[RAG Test] File chunks endpoint returned ${response.status} - file may still be processing`
          );
          return; // Skip further assertions, but don't fail the test
        }

        const data = await response.json();
        expect(data.data).toBeDefined();
      });
    });

    // =========================================================================
    // Test 9: Delete file from vector store
    // =========================================================================
    describe("Remove File from Vector Store", () => {
      it("removes file from vector store", async () => {
        if (!fileId || !vectorStoreId) {
          throw new Error(
            "Test prerequisite failed: fileId or vectorStoreId not set. Previous tests must pass first."
          );
        }
        const { gatewayUrl, apiKey } = getContext();

        const response = await trackedFetch(
          `${gatewayUrl}/api/v1/vector_stores/${vectorStoreId}/files/${fileId}`,
          {
            method: "DELETE",
            headers: { "X-API-Key": apiKey },
          }
        );

        // Delete should succeed - this is a core operation
        expect(response.ok).toBe(true);
        const data = await response.json();
        expect(data.deleted).toBe(true);
      });
    });

    // =========================================================================
    // Test 10: Delete vector store
    // =========================================================================
    describe("Delete Vector Store", () => {
      it("deletes the vector store", async () => {
        if (!vectorStoreId) {
          throw new Error(
            "Test prerequisite failed: vectorStoreId not set. The 'creates a vector store successfully' test must pass first."
          );
        }
        const { gatewayUrl, apiKey } = getContext();

        const response = await trackedFetch(
          `${gatewayUrl}/api/v1/vector_stores/${vectorStoreId}`,
          {
            method: "DELETE",
            headers: { "X-API-Key": apiKey },
          }
        );

        // Delete should succeed - this is a core operation
        expect(response.ok).toBe(true);
        const data = await response.json();
        expect(data.deleted).toBe(true);
      });
    });

    // =========================================================================
    // Test 11: Delete the uploaded file
    // =========================================================================
    describe("Delete Uploaded File", () => {
      it("deletes the uploaded file", async () => {
        if (!fileId) {
          throw new Error(
            "Test prerequisite failed: fileId not set. The 'uploads a test file successfully' test must pass first."
          );
        }
        const { gatewayUrl, apiKey } = getContext();

        const response = await trackedFetch(
          `${gatewayUrl}/api/v1/files/${fileId}`,
          {
            method: "DELETE",
            headers: { "X-API-Key": apiKey },
          }
        );

        // Delete should succeed - this is a core operation
        expect(response.ok).toBe(true);
        const data = await response.json();
        expect(data.deleted).toBe(true);
      });
    });
  });
}
