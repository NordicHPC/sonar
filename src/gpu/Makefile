# Generate shims to the Sonar C GPU libraries using bindgen.
#
# If this needs to be run and it fails you, look here:
# https://rust-lang.github.io/rust-bindgen/requirements.html

.PHONY: generate

generate: amd_smi.rs fakegpu_smi.rs habana_smi.rs nvidia_nvml.rs xpu_smi.rs

amd_smi.rs:  ../../gpuapi/sonar-amd.h Makefile
	bindgen ../../gpuapi/sonar-amd.h -o amd_smi.rs \
		--with-derive-default \
		--allowlist-function amdml_device_get_count \
		--allowlist-function amdml_device_get_card_info \
		--allowlist-function amdml_device_get_card_state \
		--allowlist-function amdml_device_probe_processes \
		--allowlist-function amdml_get_process \
		--allowlist-function amdml_free_processes

habana_smi.rs:  ../../gpuapi/sonar-habana.h Makefile
	bindgen ../../gpuapi/sonar-habana.h -o habana_smi.rs \
		--with-derive-default \
		--allowlist-function habana_device_get_count \
		--allowlist-function habana_device_get_card_info \
		--allowlist-function habana_device_get_card_state

nvidia_nvml.rs:  ../../gpuapi/sonar-nvidia.h Makefile
	bindgen ../../gpuapi/sonar-nvidia.h -o nvidia_nvml.rs \
		--with-derive-default \
		--allowlist-function nvml_device_get_count \
		--allowlist-function nvml_device_get_card_info \
		--allowlist-function nvml_device_get_card_state \
		--allowlist-function nvml_device_probe_processes \
		--allowlist-function nvml_get_process \
		--allowlist-function nvml_free_processes

fakegpu_smi.rs: ../../gpuapi/sonar-fakegpu.h Makefile
	bindgen ../../gpuapi/sonar-fakegpu.h -o fakegpu_smi.rs \
		--with-derive-default \
		--allowlist-function fakegpu_device_get_count \
		--allowlist-function fakegpu_device_get_card_info \
		--allowlist-function fakegpu_device_get_card_state \
		--allowlist-function fakegpu_device_probe_processes \
		--allowlist-function fakegpu_get_process \
		--allowlist-function fakegpu_free_processes

xpu_smi.rs:  ../../gpuapi/sonar-xpu.h Makefile
	bindgen ../../gpuapi/sonar-xpu.h -o xpu_smi.rs \
		--with-derive-default \
		--allowlist-function xpu_device_get_count \
		--allowlist-function xpu_device_get_card_info \
		--allowlist-function xpu_device_get_card_state \
		--allowlist-function xpu_device_probe_processes \
		--allowlist-function xpu_get_process \
		--allowlist-function xpu_free_processes

