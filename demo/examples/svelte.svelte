<script lang="ts">
    import { onMount } from 'svelte';

    export let name: string = 'World';
    let count = 0;
    let items: string[] = [];

    $: doubled = count * 2;
    $: if (count > 10) console.log('Count is high!');

    function increment() {
        count += 1;
    }

    onMount(() => {
        items = ['Apple', 'Banana', 'Cherry'];
    });
</script>

<main>
    <h1>Hello {name}!</h1>
    <button on:click={increment}>
        Count: {count} (doubled: {doubled})
    </button>

    <ul>
        {#each items as item, i}
            <li>{i + 1}. {item}</li>
        {/each}
    </ul>
</main>

<style>
    main { padding: 1rem; }
    button { background: #ff3e00; color: white; }
</style>
