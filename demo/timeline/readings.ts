export interface ExampleReading {
  level: number;
  charging: boolean;
  output: string;
}

const reading = (level: number, charging: boolean): ExampleReading => ({
  level, charging, output: `Battery: ${level}%${charging ? " (charging)" : ""}`,
});

/** These README examples supply every film terminal and Clicky battery value. */
export const EXAMPLE_READINGS = {
  wireless: reading(78, false),
  charging: reading(42, true),
} as const;
