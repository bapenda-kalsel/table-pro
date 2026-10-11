<script lang="ts">
  import "../app.css";
  let { children } = $props();
</script>

<div class="h-screen w-screen flex flex-col bg-app text-fg overflow-hidden select-none">
  {@render children()}
</div>
