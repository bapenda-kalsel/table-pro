<script lang="ts">
  import "../app.css";
  let { children } = $props();
</script>

<div class="min-h-[100dvh] h-[100dvh] w-screen flex flex-col bg-app text-fg overflow-hidden select-none">
  {@render children()}
</div>
