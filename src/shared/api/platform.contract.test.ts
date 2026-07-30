import { describe, expect, it } from "vitest";
import fixture from "./platform.contract.fixture.json";
import type {
  CapabilityStatusDto,
  HostPlatformDto,
  PlatformCapabilitiesDto,
  ServiceStateDto,
} from "./provision";

type CaptureEndpointDto =
  | { kind: "unix_socket"; value: string }
  | { kind: "windows_named_pipe"; value: string }
  | { kind: "unsupported" };

interface PlatformContractFixture {
  capabilities: PlatformCapabilitiesDto;
  serviceRegistration: string | null;
  desktopRegistration: string | null;
  captureEndpoint: CaptureEndpointDto;
  runtime: ServiceStateDto;
}

const hosts: HostPlatformDto[] = ["linux", "windows", "mac_os", "other"];
const statuses: CapabilityStatusDto[] = [
  "supported",
  "unsupported",
  "unavailable",
  "degraded",
];

describe("platform Rust/TypeScript contract", () => {
  it("keeps capability enums and optional platform descriptions explicit", () => {
    const states: PlatformContractFixture[] = fixture as PlatformContractFixture[];
    expect(states.map((state) => state.capabilities.host)).toEqual([
      "linux",
      "windows",
      "other",
    ]);
    for (const state of states) {
      expect(hosts).toContain(state.capabilities.host);
      for (const value of Object.values(state.capabilities).slice(1)) {
        expect(statuses).toContain(value);
      }
    }
    expect(states[1].serviceRegistration).toBeNull();
    expect(states[1].captureEndpoint.kind).toBe("windows_named_pipe");
    expect(states[1].capabilities.backgroundRuntime).toBe("supported");
    expect(states[1].capabilities.userInstallation).toBe("unsupported");
    expect(states[0].runtime.captureReady).toBe(true);
    expect(states[1].runtime.backend).toBe("windows_task_scheduler");
    expect(states[1].runtime.supported).toBe(true);
  });
});
