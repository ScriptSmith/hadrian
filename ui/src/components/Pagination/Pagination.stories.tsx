import type { Meta, StoryObj } from "@storybook/react";
import { Pagination } from "./Pagination";

const meta: Meta<typeof Pagination> = {
  title: "UI/Pagination",
  component: Pagination,
  parameters: {
    layout: "padded",
  },
  argTypes: {
    size: {
      control: "select",
      options: ["sm", "md"],
    },
    isLoading: {
      control: "boolean",
    },
  },
  args: {
    onPrevious: () => {},
    onNext: () => {},
    onFirst: () => {},
  },
};

export default meta;
type Story = StoryObj<typeof meta>;

export const FirstPage: Story = {
  args: {
    pagination: {
      limit: 25,
      has_more: true,
      next_cursor: "cursor-for-page-2",
    },
    isFirstPage: true,
    pageNumber: 1,
  },
};

export const MiddlePage: Story = {
  args: {
    pagination: {
      limit: 25,
      has_more: true,
      next_cursor: "cursor-for-page-4",
      prev_cursor: "cursor-for-page-2",
    },
    isFirstPage: false,
    pageNumber: 3,
  },
};

export const LastPage: Story = {
  args: {
    pagination: {
      limit: 25,
      has_more: false,
      prev_cursor: "cursor-for-page-4",
    },
    isFirstPage: false,
    pageNumber: 5,
  },
};

export const SinglePage: Story = {
  args: {
    pagination: {
      limit: 25,
      has_more: false,
    },
    isFirstPage: true,
    pageNumber: 1,
  },
};

export const Loading: Story = {
  args: {
    pagination: {
      limit: 25,
      has_more: true,
      next_cursor: "cursor-for-page-2",
    },
    isFirstPage: true,
    pageNumber: 1,
    isLoading: true,
  },
};

export const MediumSize: Story = {
  args: {
    pagination: {
      limit: 50,
      has_more: true,
      next_cursor: "cursor-for-page-2",
      prev_cursor: "cursor-for-page-1",
    },
    isFirstPage: false,
    pageNumber: 2,
    size: "md",
  },
};

export const WithoutFirstButton: Story = {
  args: {
    pagination: {
      limit: 25,
      has_more: true,
      next_cursor: "cursor-for-page-3",
      prev_cursor: "cursor-for-page-1",
    },
    isFirstPage: false,
    pageNumber: 2,
    onFirst: undefined,
  },
};
