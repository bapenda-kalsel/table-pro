<script lang="ts">
  import "../app.css";
  let { children } = $props();
</script>

<div class="h-screen w-screen flex flex-col bg-[#121214] text-[#e4e4e7] overflow-hidden select-none">
  {@render children()}
</div>
