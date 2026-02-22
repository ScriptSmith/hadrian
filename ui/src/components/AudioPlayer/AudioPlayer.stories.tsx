import type { Meta, StoryObj } from "@storybook/react";
import { useState, useEffect } from "react";

import { AudioPlayer } from "./AudioPlayer";
import type { PlaybackState } from "@/hooks/useAudioPlayback";

const meta = {
  title: "Components/AudioPlayer",
  component: AudioPlayer,
  parameters: {
    layout: "centered",
  },

  decorators: [
    (Story) => (
      <div className="w-[400px]">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof AudioPlayer>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Interactive player that simulates playback */
function InteractivePlayer({
  initialState = "idle",
  initialDuration = 120,
}: {
  initialState?: PlaybackState;
  initialDuration?: number;
}) {
  const [state, setState] = useState<PlaybackState>(initialState);
  const [currentTime, setCurrentTime] = useState(0);
  const [speed, setSpeed] = useState(1);
  const duration = initialDuration;

  // Simulate playback progress
  useEffect(() => {
    if (state !== "playing") return;

    const interval = setInterval(() => {
      setCurrentTime((prev) => {
        const next = prev + 0.1 * speed;
        if (next >= duration) {
          setState("idle");
          return 0;
        }
        return next;
      });
    }, 100);

    return () => clearInterval(interval);
  }, [state, speed, duration]);

  const handleTogglePlayPause = () => {
    if (state === "playing") {
      setState("paused");
    } else {
      setState("playing");
    }
  };

  const handleStop = () => {
    setState("idle");
    setCurrentTime(0);
  };

  const handleSeek = (position: number) => {
    setCurrentTime(position * duration);
  };

  return (
    <AudioPlayer
      state={state}
      currentTime={currentTime}
      duration={duration}
      progress={(currentTime / duration) * 100}
      speed={speed}
      onTogglePlayPause={handleTogglePlayPause}
      onStop={handleStop}
      onSeek={handleSeek}
      onSetSpeed={setSpeed}
      showStop
    />
  );
}

export const Default: Story = {
  args: {
    state: "idle",
    currentTime: 0,
    duration: 120,
    progress: 0,
    speed: 1,
    onTogglePlayPause: () => {},
    onStop: () => {},
    onSeek: () => {},
    onSetSpeed: () => {},
  },
};

export const Playing: Story = {
  args: {
    state: "playing",
    currentTime: 45,
    duration: 120,
    progress: 37.5,
    speed: 1,
    onTogglePlayPause: () => {},
    onStop: () => {},
    onSeek: () => {},
    onSetSpeed: () => {},
  },
};

export const Paused: Story = {
  args: {
    state: "paused",
    currentTime: 60,
    duration: 120,
    progress: 50,
    speed: 1,
    onTogglePlayPause: () => {},
    onStop: () => {},
    onSeek: () => {},
    onSetSpeed: () => {},
  },
};

export const Loading: Story = {
  args: {
    state: "loading",
    currentTime: 0,
    duration: 0,
    progress: 0,
    speed: 1,
    onTogglePlayPause: () => {},
    onStop: () => {},
    onSeek: () => {},
    onSetSpeed: () => {},
  },
};

export const WithStopButton: Story = {
  args: {
    state: "playing",
    currentTime: 30,
    duration: 120,
    progress: 25,
    speed: 1,
    showStop: true,
    onTogglePlayPause: () => {},
    onStop: () => {},
    onSeek: () => {},
    onSetSpeed: () => {},
  },
};

export const FastSpeed: Story = {
  args: {
    state: "playing",
    currentTime: 30,
    duration: 120,
    progress: 25,
    speed: 1.5,
    onTogglePlayPause: () => {},
    onStop: () => {},
    onSeek: () => {},
    onSetSpeed: () => {},
  },
};

export const NoSpeedControl: Story = {
  args: {
    state: "playing",
    currentTime: 45,
    duration: 120,
    progress: 37.5,
    speed: 1,
    showSpeed: false,
    onTogglePlayPause: () => {},
    onStop: () => {},
    onSeek: () => {},
    onSetSpeed: () => {},
  },
};

export const ShortDuration: Story = {
  args: {
    state: "playing",
    currentTime: 5,
    duration: 15,
    progress: 33.3,
    speed: 1,
    onTogglePlayPause: () => {},
    onStop: () => {},
    onSeek: () => {},
    onSetSpeed: () => {},
  },
};

export const LongDuration: Story = {
  args: {
    state: "paused",
    currentTime: 300,
    duration: 600,
    progress: 50,
    speed: 1,
    onTogglePlayPause: () => {},
    onStop: () => {},
    onSeek: () => {},
    onSetSpeed: () => {},
  },
};

export const Interactive: Story = {
  render: () => <InteractivePlayer />,
};

export const InteractiveFromPlaying: Story = {
  render: () => <InteractivePlayer initialState="playing" />,
};
