<script setup lang="ts" generic="T extends string | number">
import { Check, ChevronDown } from "lucide-vue-next";
import { computed, onBeforeUnmount, onMounted, ref } from "vue";

type MultiSelectOption<TValue extends string | number> = {
  value: TValue;
  label: string;
  meta?: string;
  disabled?: boolean;
};

const props = defineProps<{
  label: string;
  allLabel: string;
  allSelectedLabel?: string;
  selectedLabel: string;
  modelValue: T[];
  options: MultiSelectOption<T>[];
  disabled?: boolean;
}>();

const emit = defineEmits<{
  "update:modelValue": [value: T[]];
}>();

const root = ref<HTMLElement | null>(null);
const open = ref(false);

const selected = computed(() => new Set(props.modelValue));
const triggerLabel = computed(() => {
  const selectedOptions = props.options.filter((option) => selected.value.has(option.value));
  if (selectedOptions.length === 0) return props.allLabel;
  if (props.allSelectedLabel && selectedOptions.length === props.options.length) return props.allSelectedLabel;
  if (selectedOptions.length === 1) return selectedOptions[0].label;
  return props.selectedLabel.replace("{count}", String(selectedOptions.length));
});

function toggleMenu() {
  if (props.disabled) return;
  open.value = !open.value;
}

function toggleOption(option: MultiSelectOption<T>) {
  if (option.disabled) return;
  const next = new Set(props.modelValue);
  if (next.has(option.value)) next.delete(option.value);
  else next.add(option.value);
  emit("update:modelValue", props.options.map((item) => item.value).filter((value) => next.has(value)));
}

function onDocumentPointerDown(event: PointerEvent) {
  if (!root.value?.contains(event.target as Node)) open.value = false;
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") open.value = false;
}

onMounted(() => {
  document.addEventListener("pointerdown", onDocumentPointerDown);
  document.addEventListener("keydown", onKeydown);
});

onBeforeUnmount(() => {
  document.removeEventListener("pointerdown", onDocumentPointerDown);
  document.removeEventListener("keydown", onKeydown);
});
</script>

<template>
  <div ref="root" class="multi-select" :class="{ open }">
    <button type="button" class="multi-select-trigger" :disabled="disabled" :aria-expanded="open" @click="toggleMenu">
      <span class="multi-select-copy">
        <span class="multi-select-label">{{ label }}</span>
        <span class="multi-select-value">{{ triggerLabel }}</span>
      </span>
      <ChevronDown :size="16" />
    </button>
    <div v-if="open" class="multi-select-menu" role="listbox" :aria-label="label">
      <button
        v-for="option in options"
        :key="option.value"
        type="button"
        class="multi-select-option"
        :class="{ selected: selected.has(option.value) }"
        :disabled="option.disabled"
        @click="toggleOption(option)"
      >
        <span class="multi-select-option-copy">
          <span>{{ option.label }}</span>
          <small v-if="option.meta">{{ option.meta }}</small>
        </span>
        <Check v-if="selected.has(option.value)" class="multi-select-check" :size="16" />
      </button>
    </div>
  </div>
</template>
