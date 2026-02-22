import type { Meta, StoryObj } from "@storybook/react";
import { TranscriptionResult } from "./TranscriptionResult";

const meta = {
  title: "Studio/TranscriptionResult",
  component: TranscriptionResult,
  parameters: {
    layout: "centered",
  },
  decorators: [
    (Story) => (
      <div className="w-[500px]">
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof TranscriptionResult>;

export default meta;
type Story = StoryObj<typeof meta>;

export const PlainText: Story = {
  args: {
    text: "Hello, welcome to the demo. This is a sample transcription of an audio file.",
    format: "text",
  },
};

export const SrtFormat: Story = {
  args: {
    text: "1\n00:00:00,000 --> 00:00:03,000\nHello, welcome to the demo.\n\n2\n00:00:03,500 --> 00:00:07,000\nThis is a sample transcription.",
    format: "srt",
  },
};

export const VttFormat: Story = {
  args: {
    text: "WEBVTT\n\n00:00:00.000 --> 00:00:03.000\nHello, welcome to the demo.\n\n00:00:03.500 --> 00:00:07.000\nThis is a sample transcription.",
    format: "vtt",
  },
};

export const JsonFormat: Story = {
  args: {
    text: '{"text":"Hello, welcome to the demo.","segments":[{"id":0,"start":0.0,"end":3.0,"text":"Hello, welcome to the demo."},{"id":1,"start":3.5,"end":7.0,"text":"This is a sample transcription."}]}',
    format: "json",
  },
};
