package main

import "fmt"

func isPrime(n int) bool {
	if n < 2 {
		return false
	}
	for i := 2; i*i <= n; i++ {
		if n%i == 0 {
			return false
		}
	}
	return true
}

func main() {
	count := 0
	for n := 2; n < 2000000; n++ {
		if isPrime(n) {
			count++
		}
	}
	fmt.Println(count)
}
