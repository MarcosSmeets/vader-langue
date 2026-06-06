#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:/usr/bin:$PATH"
cd /mnt/c/Users/marco/Documents/workspace/side_projects/vader || exit 1
echo "=== compila os drivers (clang -c) ==="
for f in vader_pg vader_mysql; do
  if clang -c -O2 -Wall runtime/$f.c -o /tmp/$f.o 2>/tmp/$f.log; then echo "$f OK"; else echo "$f FALHOU:"; grep 'error:' /tmp/$f.log | head; fi
done
if clang -c -O2 -Iruntime/sqlite runtime/vader_db.c -o /tmp/vdb.o 2>/tmp/vdb.log; then echo "vader_db OK"; else echo "vader_db FALHOU:"; grep 'error:' /tmp/vdb.log | head; fi

echo "=== verifica a crypto (vetores conhecidos) ==="
cp runtime/vader_mysql.c /tmp/sha1t.c
printf '\nint main(){unsigned char h[20];sha1((const unsigned char*)"abc",3,h);for(int i=0;i<20;i++)printf("%%02x",h[i]);printf("\\n");return 0;}\n' >> /tmp/sha1t.c
clang /tmp/sha1t.c -o /tmp/sha1t 2>/dev/null && { printf "SHA1(abc)   = "; /tmp/sha1t; }
echo   "esperado      a9993e364706816aba3e25717850c26c9cd0d89d"
cp runtime/vader_pg.c /tmp/sha2t.c
printf '\nint main(){unsigned char h[32];sha256((const unsigned char*)"abc",3,h);for(int i=0;i<32;i++)printf("%%02x",h[i]);printf("\\n");return 0;}\n' >> /tmp/sha2t.c
clang /tmp/sha2t.c -o /tmp/sha2t 2>/dev/null && { printf "SHA256(abc) = "; /tmp/sha2t; }
echo   "esperado      ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"

echo "=== cargo build ==="
cargo build 2>&1 | grep -E '^error' | head -20
echo "=== fim ==="