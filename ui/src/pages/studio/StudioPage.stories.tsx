import { useEffect, useState, type ReactNode } from "react";
import type { Meta, StoryObj } from "@storybook/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import { http, HttpResponse } from "msw";
import { ToastProvider } from "@/components/Toast/Toast";
import StudioPage from "./StudioPage";
import type { ImageHistoryEntry, AudioHistoryEntry, TranscriptionHistoryEntry } from "./types";

// --- IndexedDB seeder ---

const DB_NAME = "hadrian-storage";
const DB_VERSION = 1;
const STORE_NAME = "keyval";

function seedIndexedDB(data: Record<string, unknown>): Promise<void> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DB_NAME, DB_VERSION);
    request.onerror = () => reject(request.error);
    request.onupgradeneeded = (event) => {
      const db = (event.target as IDBOpenDBRequest).result;
      if (!db.objectStoreNames.contains(STORE_NAME)) {
        db.createObjectStore(STORE_NAME);
      }
    };
    request.onsuccess = () => {
      const db = request.result;
      const tx = db.transaction(STORE_NAME, "readwrite");
      const store = tx.objectStore(STORE_NAME);
      for (const [key, value] of Object.entries(data)) {
        store.put(value, key);
      }
      tx.oncomplete = () => {
        db.close();
        resolve();
      };
      tx.onerror = () => {
        db.close();
        reject(tx.error);
      };
    };
  });
}

function IndexedDBSeeder({
  data,
  children,
}: {
  data: Record<string, unknown>;
  children: ReactNode;
}) {
  const [ready, setReady] = useState(false);

  useEffect(() => {
    seedIndexedDB(data).then(() => setReady(true));
  }, [data]);

  if (!ready) return <div className="p-8 text-muted-foreground">Seeding data…</div>;
  return <>{children}</>;
}

// --- Image assets (stored in public/story-assets/studio/) ---
// Generated via: http://localhost:5173/api/v1/images/generations (openai/gpt-image-1)

const IMG_LIGHTHOUSE = "story-assets/studio/lighthouse.png";

const IMG_CITY = "story-assets/studio/city.png";

const IMG_LIBRARY = "story-assets/studio/library.png";

const IMG_ASTRONAUT = "story-assets/studio/astronaut.png";

// --- Revised prompts from DALL-E 3 ---

const REVISED_PROMPT_1 =
  "A majestic lighthouse perched on a rugged, wave-battered cliff at sunset. Golden and amber light cascading across the sky, reflecting off the ocean waters below.";
const REVISED_PROMPT_2 =
  "A futuristic city skyline at night illuminated by vibrant neon lights in shades of electric blue, magenta, and cyan. Sleek skyscrapers with glowing windows and holographic advertisements.";
const REVISED_PROMPT_3 =
  "A cozy, warmly lit library with floor-to-ceiling dark oak bookshelves packed with leather-bound volumes. A plush reading chair sits beside a crackling fireplace.";
const REVISED_PROMPT_4 =
  "An astronaut in a detailed white spacesuit floating weightlessly in the vast expanse of outer space, with planet Earth glowing brilliantly in the background below.";

// --- Mock history data ---

const now = Date.now();
const hour = 3_600_000;

const imageHistory: ImageHistoryEntry[] = [
  {
    id: "img-1",
    prompt: "A majestic lighthouse on a rocky cliff at sunset",
    options: { size: "1024x1024", quality: "hd", style: "vivid", n: 1 },
    results: [
      {
        instanceId: "inst-dalle3",
        modelId: "openai/dall-e-3",
        label: "DALL-E 3",
        images: [{ imageData: IMG_LIGHTHOUSE, revisedPrompt: REVISED_PROMPT_1 }],
        costMicrocents: 8_000,
      },
    ],
    createdAt: now - hour * 2,
  },
  {
    id: "img-2",
    prompt: "A futuristic city skyline at night with neon lights",
    options: { size: "1024x1024", quality: "hd", style: "vivid", n: 1 },
    results: [
      {
        instanceId: "inst-dalle3",
        modelId: "openai/dall-e-3",
        label: "DALL-E 3",
        images: [{ imageData: IMG_CITY, revisedPrompt: REVISED_PROMPT_2 }],
        costMicrocents: 8_000,
      },
    ],
    createdAt: now - hour * 4,
  },
  {
    id: "img-3",
    prompt: "A cozy library with floor-to-ceiling bookshelves",
    options: { size: "1024x1024", quality: "standard", style: "natural", n: 1 },
    results: [
      {
        instanceId: "inst-dalle3",
        modelId: "openai/dall-e-3",
        label: "DALL-E 3",
        images: [{ imageData: IMG_LIBRARY, revisedPrompt: REVISED_PROMPT_3 }],
        costMicrocents: 4_000,
      },
    ],
    createdAt: now - hour * 8,
  },
  {
    id: "img-4",
    prompt: "An astronaut floating in space above Earth",
    options: { size: "1024x1792", quality: "hd", style: "vivid", n: 1 },
    results: [
      {
        instanceId: "inst-dalle3",
        modelId: "openai/dall-e-3",
        label: "DALL-E 3",
        images: [{ imageData: IMG_ASTRONAUT, revisedPrompt: REVISED_PROMPT_4 }],
        costMicrocents: 12_000,
      },
    ],
    createdAt: now - hour * 24,
  },
];

const audioHistory: AudioHistoryEntry[] = [
  {
    id: "audio-1",
    text: "Shall I compare thee to a summer's day? Thou art more lovely and more temperate. Rough winds do shake the darling buds of May, And summer's lease hath all too short a date.",
    options: { speed: 1.0, format: "mp3" },
    results: [
      {
        instanceId: "inst-tts1",
        modelId: "openai/tts-1",
        label: "TTS-1",
        voice: "alloy",
        audioData: "audio-1_inst-tts1.mp3",
        costMicrocents: 1_500,
      },
    ],
    createdAt: now - hour * 3,
  },
  {
    id: "audio-2",
    text: "Two roads diverged in a yellow wood, And sorry I could not travel both And be one traveler, long I stood And looked down one as far as I could To where it bent in the undergrowth.",
    options: { speed: 1.0, format: "mp3" },
    results: [
      {
        instanceId: "inst-tts1",
        modelId: "openai/tts-1",
        label: "TTS-1",
        voice: "nova",
        audioData: "audio-2_inst-tts1.mp3",
        costMicrocents: 1_500,
      },
    ],
    createdAt: now - hour * 6,
  },
  {
    id: "audio-3",
    text: "It is a truth universally acknowledged, that a single man in possession of a good fortune, must be in want of a wife.",
    options: { speed: 0.9, format: "mp3" },
    results: [
      {
        instanceId: "inst-tts1",
        modelId: "openai/tts-1",
        label: "TTS-1",
        voice: "shimmer",
        audioData: "audio-3_inst-tts1.mp3",
        costMicrocents: 1_200,
      },
    ],
    createdAt: now - hour * 12,
  },
];

const transcriptionHistory: TranscriptionHistoryEntry[] = [
  {
    id: "txn-1",
    fileName: "shakespeare-sonnet18.mp3",
    fileSize: 245_760,
    mode: "transcribe",
    options: { language: "en", responseFormat: "text", temperature: 0 },
    results: [
      {
        instanceId: "inst-whisper",
        modelId: "openai/whisper-1",
        label: "Whisper",
        resultText:
          "Shall I compare thee to a summer's day? Thou art more lovely and more temperate. Rough winds do shake the darling buds of May, And summer's lease hath all too short a date.",
        costMicrocents: 600,
      },
    ],
    createdAt: now - hour * 5,
  },
  {
    id: "txn-2",
    fileName: "frost-road-not-taken.mp3",
    fileSize: 312_000,
    mode: "transcribe",
    options: { language: "en", responseFormat: "text", temperature: 0 },
    results: [
      {
        instanceId: "inst-whisper",
        modelId: "openai/whisper-1",
        label: "Whisper",
        resultText:
          "Two roads diverged in a yellow wood, and sorry, I could not travel both, and be one traveller. Long I stood and looked down, one as far as I could to where it bent in the undergrowth.",
        costMicrocents: 750,
      },
    ],
    createdAt: now - hour * 7,
  },
  {
    id: "txn-3",
    fileName: "shakespeare-sonnet18.mp3",
    fileSize: 245_760,
    mode: "translate",
    options: {
      targetLanguage: "fr",
      responseFormat: "text",
      temperature: 0,
    },
    results: [
      {
        instanceId: "inst-whisper",
        modelId: "openai/whisper-1",
        label: "Whisper",
        resultText:
          "Dois-je te comparer à un jour d'été ? Tu es plus aimable et plus tempéré. Les vents violents secouent les tendres bourgeons de mai, et le bail de l'été a une durée bien trop courte.",
        costMicrocents: 850,
      },
    ],
    createdAt: now - hour * 9,
  },
  {
    id: "txn-4",
    fileName: "frost-road-not-taken.mp3",
    fileSize: 312_000,
    mode: "translate",
    options: {
      targetLanguage: "fr",
      responseFormat: "text",
      temperature: 0,
    },
    results: [
      {
        instanceId: "inst-whisper",
        modelId: "openai/whisper-1",
        label: "Whisper",
        resultText:
          "Deux routes divergeaient dans un bois jaune, et désolé, je ne pouvais pas parcourir les deux, et être un seul voyageur. Longtemps je me suis tenu debout et j'ai regardé aussi loin que je pouvais là où elle se perdait dans les sous-bois.",
        costMicrocents: 900,
      },
    ],
    createdAt: now - hour * 11,
  },
];

const seedData: Record<string, unknown> = {
  "studio-image-history": imageHistory,
  "studio-audio-history": audioHistory,
  "studio-transcription-history": transcriptionHistory,
};

// --- Mock models ---

const mockModels = {
  object: "list" as const,
  data: [
    {
      id: "openai/dall-e-3",
      object: "model" as const,
      owned_by: "openai",
      tasks: ["image_generation"],
    },
    {
      id: "openai/tts-1",
      object: "model" as const,
      owned_by: "openai",
      tasks: ["tts"],
      voices: ["alloy", "echo", "fable", "onyx", "nova", "shimmer"],
    },
    {
      id: "openai/whisper-1",
      object: "model" as const,
      owned_by: "openai",
      tasks: ["transcription", "translation"],
    },
    {
      id: "openai/gpt-4o",
      object: "model" as const,
      owned_by: "openai",
      tasks: ["chat"],
    },
  ],
};

// --- MSW handlers ---

const modelsHandler = http.get("*/api/v1/models", () => HttpResponse.json(mockModels));

// --- Story setup ---

const queryClient = new QueryClient({
  defaultOptions: {
    queries: { retry: false, staleTime: Infinity },
  },
});

function studioDecorator(tab?: string, mode?: string) {
  const params = new URLSearchParams();
  if (tab) params.set("tab", tab);
  if (mode) params.set("mode", mode);
  const entry = `/studio${params.toString() ? `?${params}` : ""}`;

  return function Decorator(Story: React.ComponentType) {
    return (
      <QueryClientProvider client={queryClient}>
        <ToastProvider>
          <IndexedDBSeeder data={seedData}>
            <MemoryRouter initialEntries={[entry]}>
              <Routes>
                <Route
                  path="/studio"
                  element={
                    <div className="h-screen">
                      <Story />
                    </div>
                  }
                />
              </Routes>
            </MemoryRouter>
          </IndexedDBSeeder>
        </ToastProvider>
      </QueryClientProvider>
    );
  };
}

const meta: Meta<typeof StudioPage> = {
  title: "Pages/StudioPage",
  component: StudioPage,
  parameters: {
    layout: "fullscreen",
    a11y: {
      config: {
        rules: [
          { id: "heading-order", enabled: false },
          { id: "landmark-unique", enabled: false },
        ],
      },
    },
    msw: { handlers: [modelsHandler] },
  },
};

export default meta;
type Story = StoryObj<typeof meta>;

export const Images: Story = {
  decorators: [studioDecorator()],
};

export const AudioSpeak: Story = {
  decorators: [studioDecorator("audio", "speak")],
};

export const AudioTranscribe: Story = {
  decorators: [studioDecorator("audio", "transcribe")],
};

export const AudioTranslate: Story = {
  decorators: [studioDecorator("audio", "translate")],
};
