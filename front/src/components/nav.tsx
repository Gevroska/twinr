import { Component, Show } from 'solid-js';
import { FiMenu, FiHome, FiHeart } from 'solid-icons/fi';
import { useNavigate } from '@solidjs/router';

const navBar: Component<{ isHome: boolean }> = ({ isHome }) => {
    const navigate = useNavigate();

    return (
        <>
            <Show when={isHome == true}>
                <div class="navbar sticky top-0 z-30 border-b border-base-200/70 bg-base-100/85 px-3 backdrop-blur-md">
                    <div class="navbar-start">
                        <div class="dropdown">
                            <label tabIndex={0} class="btn btn-ghost lg:hidden">
                                <FiMenu />
                            </label>
                            <ul
                                tabIndex={0}
                                class="menu menu-sm dropdown-content mt-3 z-[1] w-52 rounded-box border border-base-200 bg-base-100 p-2 text-lg shadow"
                            >
                                <li>
                                    <a href="/favorites">
                                        <FiHeart /> Favorites
                                    </a>
                                </li>
                            </ul>
                        </div>
                        <a class="btn btn-ghost normal-case text-lg" href="/">
                            <FiHome />
                        </a>
                    </div>
                    <div class="navbar-end hidden h-2 lg:flex">
                        <ul class="menu menu-horizontal px-1 text-lg">
                            <li>
                                <a href="/favorites">
                                    <FiHeart />
                                </a>
                            </li>
                        </ul>
                    </div>
                </div>
            </Show>
            <Show when={isHome == false}>
                {/* desktop */}
                <div class="hidden md:flex lg:flex">
                    <div class="navbar sticky top-0 z-30 border-b border-base-200/70 bg-base-100/85 px-3 backdrop-blur-md">
                        <div class="navbar-start">
                            <div class="dropdown">
                                <label tabIndex={0} class="btn btn-ghost lg:hidden">
                                    <FiMenu />
                                </label>
                                <ul
                                    tabIndex={0}
                                    class="menu menu-sm dropdown-content mt-3 z-[1] w-52 rounded-box border border-base-200 bg-base-100 p-2 text-lg shadow"
                                >
                                    <li>
                                        <a href="/favorites">
                                            <FiHeart /> Favorites
                                        </a>
                                    </li>
                                </ul>
                            </div>
                            <a class="btn btn-ghost normal-case text-lg" href="/">
                                <FiHome />
                            </a>
                        </div>
                        <div class="navbar-end hidden h-2 lg:flex">
                            <ul class="menu menu-horizontal px-1 text-lg">
                                <li>
                                    <a href="/favorites">
                                        <FiHeart />
                                    </a>
                                </li>
                            </ul>
                        </div>
                    </div>
                </div>
                {/* mobile */}
                <div class="fixed z-30 md:hidden lg:hidden">
                    <div class="btm-nav border-t border-base-200 bg-base-100">
                        <button onclick={() => navigate('/')}>
                            <FiHome />
                        </button>
                        <button onclick={() => navigate('/favorites')}>
                            <FiHeart />
                        </button>
                    </div>
                </div>
            </Show>
        </>
    );
};

export default navBar;
