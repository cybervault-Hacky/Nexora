"""Writes a TINY RANDOM-WEIGHT qwen2-architecture GGUF.

This is NOT Nexora Sonnet or Nexora Mini and is not an approved artefact. It exists only so
the pinned llama-server can load a real GGUF and stream tokens, to test the HTTP/SSE contract
that Nexora's client depends on. Its output is meaningless text by design."""
import sys, numpy as np, gguf

out = sys.argv[1]
rng = np.random.default_rng(0)

# GPT-2 byte-level alphabet: 256 base tokens, one per byte, so any prompt can be encoded.
def bytes_to_unicode():
    bs = list(range(ord("!"), ord("~") + 1)) + list(range(ord("¡"), ord("¬") + 1)) + list(range(ord("®"), ord("ÿ") + 1))
    cs = bs[:]; n = 0
    for b in range(256):
        if b not in bs:
            bs.append(b); cs.append(256 + n); n += 1
    return {b: chr(c) for b, c in zip(bs, cs)}
b2u = bytes_to_unicode()
tokens = [b2u[b] for b in range(256)] + ["<|endoftext|>"]
n_vocab = len(tokens)
EOS = 256
n_embd, n_head, n_layer, n_ff, ctx = 64, 4, 2, 128, 4096
head_dim = n_embd // n_head

w = gguf.GGUFWriter(out, "qwen2")
w.add_name("synthetic-random-qwen2-test")
w.add_context_length(ctx)
w.add_embedding_length(n_embd)
w.add_block_count(n_layer)
w.add_feed_forward_length(n_ff)
w.add_head_count(n_head)
w.add_head_count_kv(n_head)
w.add_layer_norm_rms_eps(1e-6)
w.add_rope_freq_base(10000.0)
w.add_rope_dimension_count(head_dim)
w.add_tokenizer_model("gpt2")
w.add_tokenizer_pre("default")
w.add_token_list(tokens)
w.add_token_types([gguf.TokenType.NORMAL] * 256 + [gguf.TokenType.CONTROL])
w.add_token_merges(["! !"])
w.add_eos_token_id(EOS)
w.add_chat_template("{% for m in messages %}<|im_start|>{{ m['role'] }}\n{{ m['content'] }}<|im_end|>\n{% endfor %}<|im_start|>assistant\n")

def t(*shape, scale=0.05):
    return (rng.standard_normal(shape) * scale).astype(np.float32)

w.add_tensor("token_embd.weight", t(n_vocab, n_embd))
w.add_tensor("output_norm.weight", np.ones(n_embd, np.float32))
w.add_tensor("output.weight", t(n_vocab, n_embd))
kv_dim = n_head * head_dim
for i in range(n_layer):
    w.add_tensor(f"blk.{i}.attn_norm.weight", np.ones(n_embd, np.float32))
    w.add_tensor(f"blk.{i}.attn_q.weight", t(n_embd, n_embd))
    w.add_tensor(f"blk.{i}.attn_q.bias", np.zeros(n_embd, np.float32))
    w.add_tensor(f"blk.{i}.attn_k.weight", t(kv_dim, n_embd))
    w.add_tensor(f"blk.{i}.attn_k.bias", np.zeros(kv_dim, np.float32))
    w.add_tensor(f"blk.{i}.attn_v.weight", t(kv_dim, n_embd))
    w.add_tensor(f"blk.{i}.attn_v.bias", np.zeros(kv_dim, np.float32))
    w.add_tensor(f"blk.{i}.attn_output.weight", t(n_embd, n_embd))
    w.add_tensor(f"blk.{i}.ffn_norm.weight", np.ones(n_embd, np.float32))
    w.add_tensor(f"blk.{i}.ffn_gate.weight", t(n_ff, n_embd))
    w.add_tensor(f"blk.{i}.ffn_up.weight", t(n_ff, n_embd))
    w.add_tensor(f"blk.{i}.ffn_down.weight", t(n_embd, n_ff))

w.write_header_to_file(); w.write_kv_data_to_file(); w.write_tensors_to_file(); w.close()
print("wrote", out)
